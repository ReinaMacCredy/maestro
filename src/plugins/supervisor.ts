import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { SessionRecord } from "../kernel/sessions.ts";

export type AttentionKind =
  | "STALLED_LEASE"
  | "REPEATED_FAILURE"
  | "DECISION_STALE"
  | "SCOPE_COLLISION";

interface AttentionWorkRow {
  cancelled_at: string | null;
  held_by: string | null;
  id: string;
  parent_id: string | null;
  state: "open" | "active" | "done";
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
  entityType: "decision" | "work";
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
  staleMinutes: number;
}

export interface AttentionService {
  scan(options: AttentionOptions): AttentionFinding[];
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
  return context.store.database
    .query<AttentionWorkRow, []>(
      "SELECT id, state, parent_id, held_by, cancelled_at FROM work ORDER BY id",
    )
    .all();
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
  const subject = work.held_by;
  const parent = work.parent_id ? workById.get(work.parent_id) : null;
  if (parent?.held_by && parent.held_by !== subject && liveSessions.has(parent.held_by)) {
    return [parent.held_by];
  }
  const peers = [...liveSessions.keys()]
    .filter((id) => id !== subject)
    .sort();
  if (peers.length > 0) return peers;
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
    if (work.state !== "active" || work.cancelled_at || !work.held_by) return [];
    const holder = liveSessions.get(work.held_by);
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
          `held by ${work.held_by}, last seen ${holder.lastSeen} ` +
          `(${minutesSince(holder.lastSeen, now)} min)`,
        evidence: `work.start #${startId}, sessions.last_seen ${holder.lastSeen}`,
        unknown: "whether the session is thinking, blocked on a tool, or gone",
        question: "reclaim, re-scope, or wait?",
        smallestAction: `maestro work show ${work.id}`,
      }),
      subjectSession: work.held_by,
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
      subjectSession: work.held_by,
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
      subjectSession: work.held_by,
      subjectWork: work.id,
      targets: ordinaryTargets(work, workById, liveSessions),
    }];
  });
}

function scopeCollisionDetections(
  works: AttentionWorkRow[],
  workById: Map<string, AttentionWorkRow>,
  liveSessions: Map<string, SessionRecord>,
): Detection[] {
  const active = works.filter(
    (work) =>
      work.state === "active" && !work.cancelled_at && work.parent_id && work.held_by &&
      liveSessions.has(work.held_by),
  );
  const detections: Detection[] = [];
  for (let leftIndex = 0; leftIndex < active.length; leftIndex += 1) {
    const left = active[leftIndex] as AttentionWorkRow;
    for (let rightIndex = leftIndex + 1; rightIndex < active.length; rightIndex += 1) {
      const right = active[rightIndex] as AttentionWorkRow;
      if (left.parent_id !== right.parent_id || left.held_by === right.held_by) continue;
      const pair = [left, right].sort((a, b) => a.id.localeCompare(b.id));
      const first = pair[0] as AttentionWorkRow;
      const second = pair[1] as AttentionWorkRow;
      const holders = [first.held_by as string, second.held_by as string].sort();
      const parent = first.parent_id ? workById.get(first.parent_id) : null;
      const targets =
        parent?.held_by && !holders.includes(parent.held_by) && liveSessions.has(parent.held_by)
          ? [parent.held_by]
          : holders;
      detections.push({
        entityId: first.id,
        entityType: "work",
        fingerprint: `collision:${first.id}:${second.id}:${holders[0]}:${holders[1]}`,
        kind: "SCOPE_COLLISION",
        packet: packet("SCOPE_COLLISION", `${first.id},${second.id}`, {
          observed: `siblings held by ${first.held_by} and ${second.held_by}`,
          evidence: `parent ${first.parent_id}; both work items active; both sessions live`,
          unknown: "whether the scopes intentionally overlap or compete for the same mutation",
          question: "keep both lanes, split scope, or release one lease?",
          smallestAction: `maestro work show ${first.parent_id}`,
        }),
        subjectSession: holders.join(","),
        subjectWork: first.id,
        targets,
      });
    }
  }
  return detections;
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
    ...scopeCollisionDetections(works, workById, liveSessions),
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
  });
}

const attentionFlags = {
  "--stale": { description: "Stalled lease threshold in minutes (default 30).", value: true },
  "--decision-stale": {
    description: "Draft decision threshold in hours (default 24).",
    value: true,
  },
} as const;

export const supervisorPlugin: BuiltInPlugin = {
  name: "supervisor",
  inject: ["work", "mailbox"],
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
  },
};
