import { closeSync, openSync } from "node:fs";
import { readFile, rename, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { SessionRecord } from "../kernel/sessions.ts";
import type { DispatchService } from "./dispatch.ts";
import { readInstallStamp } from "./install-stamp.ts";
import type { WorkService } from "./work.ts";

export type AttentionKind =
  | "STALLED_LEASE"
  | "REPEATED_FAILURE"
  | "DECISION_STALE"
  | "SCOPE_COLLISION"
  | "DISPATCH_UNRETURNED";

interface AttentionWorkRow {
  heldBy: string | null;
  id: string;
  parentId: string | null;
  state: "open" | "active" | "done" | "cancelled";
}

interface AttentionRow {
  created_at: string;
  fingerprint: string;
  id: number;
  kind: AttentionKind;
  packet: string;
  subject_session: string | null;
  subject_work: string | null;
  target_session: string | null;
}

interface EventRow {
  created_at: string;
  id: number;
}

interface FailedNoteRow {
  created_at: string;
  id: number;
}

interface Mailbox {
  send(targetSession: string, text: string): unknown;
}

interface Detection {
  entityId: string;
  entityType: "decision" | "dispatch" | "work";
  fingerprint: string;
  kind: AttentionKind;
  packet: string;
  subjectSession: string | null;
  subjectWork: string | null;
  targets: string[];
}

export interface AttentionFinding {
  fingerprint: string;
  kind: AttentionKind;
  packet: string;
  raised: boolean;
  raisedAt: string;
  subjectSession: string | null;
  subjectWork: string | null;
  targets: string[];
}

export interface AttentionOptions {
  decisionStaleHours: number;
  dispatchStaleHours: number;
  staleMinutes: number;
}

export interface AttentionService {
  scan(options: AttentionOptions): AttentionFinding[];
}

interface SupervisorState {
  decisionStale: number;
  dispatchStale: number;
  interval: number;
  lastRaised: number;
  lastTick: string | null;
  notify: boolean;
  pid: number;
  runtimeCommit: string;
  stale: number;
  startedAt: string;
}

interface SupervisorPaths {
  log: string;
  state: string;
}

function numericOption(
  invocation: CliInvocation,
  name: string,
  fallback: number,
): number {
  const value = invocation.options[name];
  if (value === undefined) return fallback;
  const parsed = typeof value === "string" ? Number(value) : Number.NaN;
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new CliError("INVALID_VALUE", `--${name} must be a positive number`);
  }
  return parsed;
}

function workRows(context: PluginContext): AttentionWorkRow[] {
  return (context.work as WorkService)
    .snapshot()
    .map((work) => ({
      heldBy: work.heldBy,
      id: work.id,
      parentId: work.parentId,
      state: work.state,
    }));
}

function latestStart(context: PluginContext, workId: string): EventRow | null {
  return context.store.database
    .query<EventRow, [string]>(
      `SELECT id, created_at FROM event_log
       WHERE type = 'work.start' AND entity_type = 'work' AND entity_id = ?
       ORDER BY id DESC LIMIT 1`,
    )
    .get(workId) ?? null;
}

function minutesSince(iso: string, now: number): number {
  return Math.max(0, Math.floor((now - Date.parse(iso)) / 60_000));
}

function packet(
  kind: AttentionKind,
  subject: string,
  fields: {
    evidence: string;
    observed: string;
    question: string;
    smallestAction: string;
    unknown: string;
  },
): string {
  return [
    `attention ${kind} ${subject}`,
    `  observed: ${fields.observed}`,
    `  evidence: ${fields.evidence}`,
    `  unknown: ${fields.unknown}`,
    `  question: ${fields.question}`,
    `  smallest action: ${fields.smallestAction}`,
    "  human decision needed: no",
  ].join("\n");
}

function liveSessionMap(sessions: SessionRecord[]): Map<string, SessionRecord> {
  return new Map(sessions.filter((session) => session.live).map((session) => [session.id, session]));
}

function ordinaryTargets(
  work: AttentionWorkRow,
  workById: Map<string, AttentionWorkRow>,
  liveSessions: Map<string, SessionRecord>,
): string[] {
  const subject = work.heldBy;
  const parent = work.parentId ? workById.get(work.parentId) : null;
  if (parent?.heldBy && parent.heldBy !== subject && liveSessions.has(parent.heldBy)) {
    return [parent.heldBy];
  }
  const peer = [...liveSessions.values()]
    .filter((session) => session.id !== subject)
    .sort((left, right) => {
      const recency = Date.parse(right.lastSeen) - Date.parse(left.lastSeen);
      return recency !== 0 ? recency : left.id.localeCompare(right.id);
    })[0];
  if (peer) return [peer.id];
  return subject && liveSessions.has(subject) ? [subject] : [];
}

function stalledDetections(
  context: PluginContext,
  works: AttentionWorkRow[],
  workById: Map<string, AttentionWorkRow>,
  liveSessions: Map<string, SessionRecord>,
  now: number,
  staleMinutes: number,
): Detection[] {
  const cutoff = now - staleMinutes * 60_000;
  return works.flatMap((work): Detection[] => {
    if (work.state !== "active" || !work.heldBy) return [];
    const holder = liveSessions.get(work.heldBy);
    if (!holder || Date.parse(holder.lastSeen) >= cutoff) return [];
    const start = latestStart(context, work.id);
    const startId = start?.id ?? 0;
    return [{
      entityId: work.id,
      entityType: "work",
      fingerprint: `stalled:${work.id}:${startId}`,
      kind: "STALLED_LEASE",
      packet: packet("STALLED_LEASE", work.id, {
        observed:
          `held by ${work.heldBy}, last seen ${holder.lastSeen} ` +
          `(${minutesSince(holder.lastSeen, now)} min)`,
        evidence: `work.start #${startId}, sessions.last_seen ${holder.lastSeen}`,
        unknown: "whether the session is thinking, blocked on a tool, or gone",
        question: "reclaim, re-scope, or wait?",
        // Observe and ask: the holder answers faster than the row explains.
        smallestAction: `maestro msg send ${work.heldBy} "still on ${work.id}?"`,
      }),
      subjectSession: work.heldBy,
      subjectWork: work.id,
      targets: ordinaryTargets(work, workById, liveSessions),
    }];
  });
}

function repeatedFailureDetections(
  context: PluginContext,
  works: AttentionWorkRow[],
  workById: Map<string, AttentionWorkRow>,
  liveSessions: Map<string, SessionRecord>,
): Detection[] {
  return works.flatMap((work): Detection[] => {
    if (work.state === "done" || work.state === "cancelled") return [];
    const start = latestStart(context, work.id);
    const notes = start
      ? context.store.database
          .query<FailedNoteRow, [string, string]>(
            `SELECT id, created_at FROM work_notes
             WHERE work_id = ? AND created_at >= ? AND substr(text, 1, 8) = 'failed: '
             ORDER BY id`,
          )
          .all(work.id, start.created_at)
      : context.store.database
          .query<FailedNoteRow, [string]>(
            `SELECT id, created_at FROM work_notes
             WHERE work_id = ? AND substr(text, 1, 8) = 'failed: '
             ORDER BY id`,
          )
          .all(work.id);
    const third = notes[2];
    if (!third) return [];
    const startEvidence = start ? `work.start #${start.id}` : "no work.start on record";
    return [{
      entityId: work.id,
      entityType: "work",
      fingerprint: `repeat:${work.id}:${third.id}`,
      kind: "REPEATED_FAILURE",
      packet: packet("REPEATED_FAILURE", work.id, {
        observed: `${notes.length} failed passes since the latest lease`,
        evidence: `${startEvidence}; third failed note #${third.id}`,
        unknown: "whether the failures share one mechanism or need a new decision",
        question: "inspect the episode, re-scope, or revisit the decision?",
        smallestAction: `maestro work show ${work.id}`,
      }),
      subjectSession: work.heldBy,
      subjectWork: work.id,
      targets: ordinaryTargets(work, workById, liveSessions),
    }];
  });
}

function decisionStaleDetections(
  context: PluginContext,
  workById: Map<string, AttentionWorkRow>,
  liveSessions: Map<string, SessionRecord>,
  now: number,
  decisionStaleHours: number,
): Detection[] {
  const cutoff = new Date(now - decisionStaleHours * 60 * 60_000).toISOString();
  const rows = context.store.database
    .query<{ created_at: string; id: string; work_id: string }, [string]>(
      `SELECT decisions.id, decisions.created_at, decisions.work_id
       FROM decisions
       JOIN work ON work.id = decisions.work_id
       WHERE decisions.state = 'draft'
         AND decisions.created_at < ?
         AND work.state != 'done'
         AND work.cancelled_at IS NULL
       ORDER BY decisions.id`,
    )
    .all(cutoff);
  return rows.flatMap((decision): Detection[] => {
    const work = workById.get(decision.work_id);
    if (!work) return [];
    return [{
      entityId: decision.id,
      entityType: "decision",
      fingerprint: `decision:${decision.id}`,
      kind: "DECISION_STALE",
      packet: packet("DECISION_STALE", decision.id, {
        observed: `draft linked to ${decision.work_id} remains open`,
        evidence: `decisions.created_at ${decision.created_at}`,
        unknown: "whether the fork is still active, blocked, or abandoned",
        question: "lock, supersede, re-scope, or keep investigating?",
        smallestAction: `maestro decision show ${decision.id}`,
      }),
      subjectSession: work.heldBy,
      subjectWork: work.id,
      targets: ordinaryTargets(work, workById, liveSessions),
    }];
  });
}

// work.start prints an [overlap] banner to the session that starts second, so
// only the earlier holder is missing the fact that a sibling lane opened.
function earlierHolder(
  context: PluginContext,
  first: AttentionWorkRow,
  second: AttentionWorkRow,
): string {
  const firstStart = latestStart(context, first.id)?.id ?? 0;
  const secondStart = latestStart(context, second.id)?.id ?? 0;
  const earlier = firstStart <= secondStart ? first : second;
  return earlier.heldBy as string;
}

function scopeCollisionDetections(
  context: PluginContext,
  works: AttentionWorkRow[],
  workById: Map<string, AttentionWorkRow>,
  liveSessions: Map<string, SessionRecord>,
): Detection[] {
  const active = works.filter(
    (work) =>
      work.state === "active" && work.parentId && work.heldBy &&
      liveSessions.has(work.heldBy),
  );
  const detections: Detection[] = [];
  for (let leftIndex = 0; leftIndex < active.length; leftIndex += 1) {
    const left = active[leftIndex] as AttentionWorkRow;
    for (let rightIndex = leftIndex + 1; rightIndex < active.length; rightIndex += 1) {
      const right = active[rightIndex] as AttentionWorkRow;
      if (left.parentId !== right.parentId || left.heldBy === right.heldBy) continue;
      const pair = [left, right].sort((a, b) => a.id.localeCompare(b.id));
      const first = pair[0] as AttentionWorkRow;
      const second = pair[1] as AttentionWorkRow;
      const holders = [first.heldBy as string, second.heldBy as string].sort();
      const parent = first.parentId ? workById.get(first.parentId) : null;
      const targets =
        parent?.heldBy && !holders.includes(parent.heldBy) && liveSessions.has(parent.heldBy)
          ? [parent.heldBy]
          : [earlierHolder(context, first, second)];
      detections.push({
        entityId: first.id,
        entityType: "work",
        fingerprint: `collision:${first.id}:${second.id}:${holders[0]}:${holders[1]}`,
        kind: "SCOPE_COLLISION",
        packet: packet("SCOPE_COLLISION", `${first.id},${second.id}`, {
          observed: `siblings held by ${first.heldBy} and ${second.heldBy}`,
          evidence: `parent ${first.parentId}; both work items active; both sessions live`,
          unknown: "whether the scopes intentionally overlap or compete for the same mutation",
          question: "keep both lanes, split scope, or release one lease?",
          smallestAction: `maestro work show ${first.parentId}`,
        }),
        subjectSession: holders.join(","),
        subjectWork: first.id,
        targets,
      });
    }
  }
  return detections;
}

function dispatchUnreturnedDetections(
  context: PluginContext,
  workById: Map<string, AttentionWorkRow>,
  liveSessions: Map<string, SessionRecord>,
  now: number,
  dispatchStaleHours: number,
): Detection[] {
  const cutoff = now - dispatchStaleHours * 60 * 60_000;
  const dispatch = context.dispatch as DispatchService;
  return dispatch.list().flatMap((record): Detection[] => {
    if (record.state !== "open" || Date.parse(record.createdAt) >= cutoff) return [];
    const work = workById.get(record.workId);
    if (!work || work.state === "done" || work.state === "cancelled") return [];
    const subjectSession = record.heldBy ?? record.targetSession;
    const routingWork = { ...work, heldBy: subjectSession };
    return [{
      entityId: record.id,
      entityType: "dispatch",
      fingerprint: `dispatch-unreturned:${record.id}`,
      kind: "DISPATCH_UNRETURNED",
      packet: packet("DISPATCH_UNRETURNED", record.id, {
        observed:
          `dispatch for ${record.workId} has no handback after ` +
          `${minutesSince(record.createdAt, now)} minutes`,
        evidence: `dispatches.created_at ${record.createdAt}; no handbacks row`,
        unknown: "whether the lane is working, blocked, or abandoned",
        question: "wait, contact the lane, cancel it, or re-scope?",
        smallestAction: subjectSession
          ? `maestro msg send ${subjectSession} "still on ${record.id}?"`
          : `maestro dispatch show ${record.id}`,
      }),
      subjectSession,
      subjectWork: record.workId,
      targets: ordinaryTargets(routingWork, workById, liveSessions),
    }];
  });
}

function detect(context: PluginContext, options: AttentionOptions): Detection[] {
  const now = Date.now();
  const works = workRows(context);
  const workById = new Map(works.map((work) => [work.id, work]));
  const liveSessions = liveSessionMap(context.sessions.list());
  return [
    ...stalledDetections(
      context,
      works,
      workById,
      liveSessions,
      now,
      options.staleMinutes,
    ),
    ...repeatedFailureDetections(context, works, workById, liveSessions),
    ...decisionStaleDetections(
      context,
      workById,
      liveSessions,
      now,
      options.decisionStaleHours,
    ),
    ...scopeCollisionDetections(context, works, workById, liveSessions),
    ...dispatchUnreturnedDetections(
      context,
      workById,
      liveSessions,
      now,
      options.dispatchStaleHours,
    ),
  ];
}

function raise(context: PluginContext, detection: Detection): AttentionFinding {
  const existing = context.store.database
    .query<AttentionRow, [string]>("SELECT * FROM attention WHERE fingerprint = ?")
    .get(detection.fingerprint);
  if (existing) {
    return {
      fingerprint: detection.fingerprint,
      kind: detection.kind,
      packet: detection.packet,
      raised: false,
      raisedAt: existing.created_at,
      subjectSession: detection.subjectSession,
      subjectWork: detection.subjectWork,
      targets: detection.targets,
    };
  }

  const createdAt = new Date().toISOString();
  let inserted = false;
  const transaction = context.store.database.transaction(() => {
    const result = context.store.database
      .query(
        `INSERT OR IGNORE INTO attention
          (kind, fingerprint, subject_work, subject_session, target_session, packet, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        detection.kind,
        detection.fingerprint,
        detection.subjectWork,
        detection.subjectSession,
        detection.targets.join(",") || null,
        detection.packet,
        createdAt,
      );
    if (result.changes === 0) return;
    inserted = true;
    context.log.append({
      type: "attention.raise",
      entityType: detection.entityType,
      entityId: detection.entityId,
      sessionId: context.sessions.current().id,
      payload: {
        fingerprint: detection.fingerprint,
        kind: detection.kind,
        targets: detection.targets,
      },
    });
    const mailbox = context.mailbox as Mailbox;
    for (const target of detection.targets) mailbox.send(target, detection.packet);
  });
  transaction();
  if (!inserted) {
    const raced = context.store.database
      .query<AttentionRow, [string]>("SELECT * FROM attention WHERE fingerprint = ?")
      .get(detection.fingerprint) as AttentionRow;
    return {
      fingerprint: detection.fingerprint,
      kind: detection.kind,
      packet: detection.packet,
      raised: false,
      raisedAt: raced.created_at,
      subjectSession: detection.subjectSession,
      subjectWork: detection.subjectWork,
      targets: detection.targets,
    };
  }
  return {
    fingerprint: detection.fingerprint,
    kind: detection.kind,
    packet: detection.packet,
    raised: true,
    raisedAt: createdAt,
    subjectSession: detection.subjectSession,
    subjectWork: detection.subjectWork,
    targets: detection.targets,
  };
}

function formatFindings(findings: AttentionFinding[]): string {
  if (findings.length === 0) return "no attention findings";
  return findings
    .map((finding) => `${finding.packet}\n  raised ${finding.raisedAt}`)
    .join("\n\n");
}

function scanFromInvocation(
  service: AttentionService,
  invocation: CliInvocation,
): AttentionFinding[] {
  return service.scan({
    staleMinutes: numericOption(invocation, "stale", 30),
    decisionStaleHours: numericOption(invocation, "decision-stale", 24),
    dispatchStaleHours: numericOption(invocation, "dispatch-stale", 2),
  });
}

const attentionFlags = {
  "--stale": { description: "Stalled lease threshold in minutes (default 30).", value: true },
  "--decision-stale": {
    description: "Draft decision threshold in hours (default 24).",
    value: true,
  },
  "--dispatch-stale": {
    description: "Unreturned dispatch threshold in hours (default 2).",
    value: true,
  },
} as const;

const supervisorFlags = {
  "--interval": { description: "Scan interval in seconds (default 60).", value: true },
  "--stale": { description: "Stalled lease threshold in minutes (default 30).", value: true },
  "--decision-stale": {
    description: "Draft decision threshold in hours (default 24).",
    value: true,
  },
  "--dispatch-stale": {
    description: "Unreturned dispatch threshold in hours (default 2).",
    value: true,
  },
  "--notify": { description: "Show one macOS notification per newly raised packet." },
} as const;

function supervisorPaths(context: PluginContext): SupervisorPaths {
  const directory = dirname(context.store.path);
  return {
    log: join(directory, "supervisor.log"),
    state: join(directory, "supervisor.json"),
  };
}

async function readSupervisorState(path: string): Promise<SupervisorState | null> {
  let text: string;
  try {
    text = await readFile(path, "utf8");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw error;
  }
  try {
    const state = JSON.parse(text) as Partial<SupervisorState>;
    if (
      typeof state.pid !== "number" ||
      typeof state.startedAt !== "string" ||
      typeof state.interval !== "number" ||
      typeof state.stale !== "number" ||
      typeof state.decisionStale !== "number" ||
      !(state.dispatchStale === undefined || typeof state.dispatchStale === "number") ||
      typeof state.notify !== "boolean" ||
      typeof state.runtimeCommit !== "string" ||
      !(state.lastTick === null || typeof state.lastTick === "string") ||
      typeof state.lastRaised !== "number"
    ) {
      throw new Error("missing required fields");
    }
    return { ...state, dispatchStale: state.dispatchStale ?? 2 } as SupervisorState;
  } catch (error) {
    throw new CliError(
      "INVALID_SUPERVISOR_STATE",
      `invalid supervisor pid file ${path}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

async function writeSupervisorState(path: string, state: SupervisorState): Promise<void> {
  const staged = `${path}.${process.pid}.tmp`;
  await writeFile(staged, `${JSON.stringify(state)}\n`);
  await rename(staged, path);
}

function pidAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === "EPERM";
  }
}

async function runtimeCommit(): Promise<string> {
  const root = resolve(import.meta.dir, "..", "..");
  const stamp = await readInstallStamp(root);
  return stamp.status === "valid" ? stamp.stamp.commit : "source";
}

function statusText(state: SupervisorState | null, currentCommit: string): string {
  if (!state) return "supervisor stopped";
  const status = pidAlive(state.pid) ? "running" : "stale";
  const lines = [
    `supervisor ${status}`,
    `pid: ${state.pid}`,
    `started: ${state.startedAt}`,
    `interval: ${state.interval}s`,
    `dispatch stale: ${state.dispatchStale}h`,
    `last tick: ${state.lastTick ?? "never"}`,
    `raised: ${state.lastRaised}`,
    `daemon commit: ${state.runtimeCommit}`,
  ];
  if (state.runtimeCommit !== currentCommit) {
    lines.push(
      `runtime drift: daemon ${state.runtimeCommit} · runtime ${currentCommit}; restart to pick up`,
    );
  }
  if (status === "stale") lines.push("run: maestro supervisor stop");
  return lines.join("\n");
}

function notifyFindings(findings: AttentionFinding[]): void {
  for (const finding of findings.filter((candidate) => candidate.raised)) {
    if (process.platform !== "darwin") {
      process.stdout.write(`notification skipped: ${finding.kind} ${finding.subjectWork ?? ""}\n`);
      continue;
    }
    const title = `maestro ${finding.kind}`;
    const body = finding.packet.split("\n")[0] ?? finding.kind;
    Bun.spawnSync([
      "osascript",
      "-e",
      `display notification ${JSON.stringify(body)} with title ${JSON.stringify(title)}`,
    ]);
  }
}

function waitForTickOrStop(
  seconds: number,
  installWake: (wake: (() => void) | null) => void,
): Promise<void> {
  return new Promise((resolveTick) => {
    const timer = setTimeout(() => {
      installWake(null);
      resolveTick();
    }, seconds * 1_000);
    installWake(() => {
      clearTimeout(timer);
      installWake(null);
      resolveTick();
    });
  });
}

async function runSupervisorLoop(
  context: PluginContext,
  service: AttentionService,
  options: {
    decisionStale: number;
    dispatchStale: number;
    interval: number;
    notify: boolean;
    stale: number;
  },
): Promise<void> {
  const paths = supervisorPaths(context);
  let stopping = false;
  let wake: (() => void) | null = null;
  const onSignal = () => {
    stopping = true;
    wake?.();
  };
  process.on("SIGTERM", onSignal);
  try {
    const commit = await runtimeCommit();
    const existing = await readSupervisorState(paths.state);
    let state: SupervisorState =
      existing?.pid === process.pid
        ? existing
        : {
            pid: process.pid,
            startedAt: new Date().toISOString(),
            interval: options.interval,
            stale: options.stale,
            decisionStale: options.decisionStale,
            dispatchStale: options.dispatchStale,
            notify: options.notify,
            runtimeCommit: commit,
            lastTick: null,
            lastRaised: 0,
          };
    do {
      const findings = service.scan({
        staleMinutes: options.stale,
        decisionStaleHours: options.decisionStale,
        dispatchStaleHours: options.dispatchStale,
      });
      if (options.notify) notifyFindings(findings);
      state = {
        ...state,
        lastTick: new Date().toISOString(),
        lastRaised: findings.filter((finding) => finding.raised).length,
      };
      await writeSupervisorState(paths.state, state);
      if (stopping) break;
      await waitForTickOrStop(options.interval, (nextWake) => {
        wake = nextWake;
      });
    } while (!stopping);
  } finally {
    process.off("SIGTERM", onSignal);
  }
}

export const supervisorPlugin: BuiltInPlugin = {
  name: "supervisor",
  inject: ["work", "dispatch", "mailbox"],
  apply(context) {
    context.store.migrate(`
      CREATE TABLE IF NOT EXISTS attention (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        kind TEXT NOT NULL,
        fingerprint TEXT NOT NULL UNIQUE,
        subject_work TEXT,
        subject_session TEXT,
        target_session TEXT,
        packet TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
    `);
    const service: AttentionService = {
      scan: (options) => detect(context, options).map((detection) => raise(context, detection)),
    };
    context.effect(() => context.provide("attention", service));
    context.effect(() =>
      context.cli.register(
        "attention",
        (invocation): CliResult => {
          const findings = scanFromInvocation(service, invocation);
          return { data: { detections: findings }, text: formatFindings(findings) };
        },
        {
          description: "Scan store state for attention packets without mutating work.",
          flags: attentionFlags,
          rootDescription: "Raise one-shot attention packets from store evidence.",
        },
      ),
    );
    context.effect(() =>
      context.cli.register(
        "attention --json",
        (invocation): CliResult => {
          const data = { detections: scanFromInvocation(service, invocation) };
          return { data, text: JSON.stringify({ ok: true, data }) };
        },
        {
          description: "Scan store state and emit one compact JSON success envelope.",
          flags: attentionFlags,
        },
      ),
    );
    context.effect(() =>
      context.cli.register(
        "supervisor start",
        async (invocation): Promise<CliResult> => {
          const paths = supervisorPaths(context);
          const currentCommit = await runtimeCommit();
          const current = await readSupervisorState(paths.state);
          if (current && pidAlive(current.pid)) {
            throw new CliError("SUPERVISOR_RUNNING", statusText(current, currentCommit), {
              pid: current.pid,
            });
          }
          const interval = numericOption(invocation, "interval", 60);
          const stale = numericOption(invocation, "stale", 30);
          const decisionStale = numericOption(invocation, "decision-stale", 24);
          const dispatchStale = numericOption(invocation, "dispatch-stale", 2);
          const notify = invocation.options.notify === true;
          const cliPath = resolve(process.argv[1] ?? join(import.meta.dir, "..", "..", "bin", "maestro.ts"));
          const command = [
            process.execPath,
            cliPath,
            "supervisor",
            "run",
            "--interval",
            String(interval),
            "--stale",
            String(stale),
            "--decision-stale",
            String(decisionStale),
            "--dispatch-stale",
            String(dispatchStale),
            ...(notify ? ["--notify"] : []),
          ];
          const descriptor = openSync(paths.log, "a");
          let child: ReturnType<typeof Bun.spawn> | null = null;
          try {
            child = Bun.spawn(command, {
              cwd: process.cwd(),
              detached: true,
              env: { ...process.env, MAESTRO_SESSION_NONE: "1" },
              stdin: "ignore",
              stdout: descriptor,
              stderr: descriptor,
            });
            child.unref();
          } finally {
            closeSync(descriptor);
          }
          const state: SupervisorState = {
            pid: child.pid,
            startedAt: new Date().toISOString(),
            interval,
            stale,
            decisionStale,
            dispatchStale,
            notify,
            runtimeCommit: currentCommit,
            lastTick: null,
            lastRaised: 0,
          };
          try {
            await writeSupervisorState(paths.state, state);
          } catch (error) {
            child.kill("SIGTERM");
            throw error;
          }
          return {
            data: { state },
            text: `supervisor started\npid: ${state.pid}\ninterval: ${state.interval}s`,
          };
        },
        {
          description: "Start one detached attention loop; never started automatically.",
          flags: supervisorFlags,
          rootDescription: "Start, stop, and inspect the opt-in attention daemon.",
        },
      ),
    );
    context.effect(() =>
      context.cli.register(
        "supervisor status",
        async (): Promise<CliResult> => {
          const state = await readSupervisorState(supervisorPaths(context).state);
          const currentCommit = await runtimeCommit();
          return { data: { state }, text: statusText(state, currentCommit) };
        },
        { description: "Report stopped, running, or stale without starting anything." },
      ),
    );
    context.effect(() =>
      context.cli.register(
        "supervisor stop",
        async (): Promise<CliResult> => {
          const paths = supervisorPaths(context);
          const state = await readSupervisorState(paths.state);
          if (!state) return { data: { state: "stopped" }, text: "supervisor stopped" };
          if (pidAlive(state.pid)) {
            try {
              process.kill(state.pid, "SIGTERM");
            } catch (error) {
              if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error;
            }
            const deadline = Date.now() + 2_000;
            while (pidAlive(state.pid) && Date.now() < deadline) await Bun.sleep(50);
            if (pidAlive(state.pid)) {
              throw new CliError(
                "SUPERVISOR_STOP_TIMEOUT",
                `supervisor did not exit (pid ${state.pid}); run: kill -9 ${state.pid}`,
                { pid: state.pid },
              );
            }
          }
          await rm(paths.state, { force: true });
          return {
            data: { pid: state.pid, state: "stopped" },
            text: `supervisor stopped\npid: ${state.pid}`,
          };
        },
        { description: "Stop the recorded daemon and remove its pid file." },
      ),
    );
    context.effect(() =>
      context.cli.register(
        "supervisor run",
        async (invocation): Promise<CliResult> => {
          await runSupervisorLoop(context, service, {
            interval: numericOption(invocation, "interval", 60),
            stale: numericOption(invocation, "stale", 30),
            decisionStale: numericOption(invocation, "decision-stale", 24),
            dispatchStale: numericOption(invocation, "dispatch-stale", 2),
            notify: invocation.options.notify === true,
          });
          return { data: { state: "stopped" }, text: "supervisor run stopped" };
        },
        {
          description: "Internal in-process attention loop used by supervisor start.",
          flags: supervisorFlags,
        },
      ),
    );
  },
};
