import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { SessionRecord } from "../kernel/sessions.ts";
import type { BriefService } from "./coordination.ts";
import type { DispatchService, HandbackService } from "./dispatch.ts";
import type { WorkService } from "./work.ts";

export type AttentionKind =
  | "STALLED_LEASE"
  | "REPEATED_FAILURE"
  | "DECISION_STALE"
  | "HUMAN_DECISION_REQUIRED"
  | "LEAD_COLLISION"
  | "SCOPE_COLLISION"
  | "DISPATCH_UNACCEPTED"
  | "DISPATCH_UNRETURNED"
  | "HANDBACK_UNREVIEWED";

interface AttentionWorkRow {
  heldBy: string | null;
  id: string;
  parentId: string | null;
  state: "open" | "active" | "done" | "cancelled";
}

interface AttentionRow {
  created_at: string;
}

interface EventRow {
  created_at: string;
  id: number;
}

interface FailedNoteRow {
  created_at: string;
  id: number;
}

interface Detection {
  entityId: string;
  entityType: "decision" | "dispatch" | "work";
  fingerprint: string;
  holderRole?: "lead" | "peer";
  kind: AttentionKind;
  packet: string;
  route?: "lead" | "supervisor";
  subjectSession: string | null;
  subjectWork: string | null;
}

export interface AttentionFinding {
  fingerprint: string;
  holderRole?: "lead" | "peer";
  kind: AttentionKind;
  packet: string;
  raised: boolean;
  raisedAt: string;
  route?: "lead" | "supervisor";
  subjectSession: string | null;
  subjectWork: string | null;
}

export interface AttentionOptions {
  decisionStaleHours: number;
  dispatchStaleHours: number;
  staleMinutes: number;
}

export interface AttentionService {
  scan(options: AttentionOptions): AttentionFinding[];
}

const subjectKind: Record<AttentionKind, "decision" | "dispatch" | "work"> = {
  STALLED_LEASE: "work",
  REPEATED_FAILURE: "work",
  DECISION_STALE: "decision",
  HUMAN_DECISION_REQUIRED: "decision",
  LEAD_COLLISION: "work",
  SCOPE_COLLISION: "work",
  DISPATCH_UNACCEPTED: "dispatch",
  DISPATCH_UNRETURNED: "dispatch",
  HANDBACK_UNREVIEWED: "dispatch",
};

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
    holderRole?: "lead" | "peer";
    humanDecision?: boolean;
    observed: string;
    question: string;
    smallestAction: string;
    unknown: string;
  },
): string {
  return [
    `attention ${kind} ${subjectKind[kind]} ${subject}`,
    `  observed: ${fields.observed}`,
    ...(fields.holderRole ? [`  holder role: ${fields.holderRole}`] : []),
    `  evidence: ${fields.evidence}`,
    `  unknown: ${fields.unknown}`,
    `  question: ${fields.question}`,
    `  smallest action: ${fields.smallestAction}`,
    `  human decision needed: ${fields.humanDecision ? "yes" : "no"}`,
  ].join("\n");
}

function sessionMap(sessions: SessionRecord[]): Map<string, SessionRecord> {
  return new Map(sessions.map((session) => [session.id, session]));
}

function stalledDetections(
  context: PluginContext,
  works: AttentionWorkRow[],
  sessions: Map<string, SessionRecord>,
  now: number,
  staleMinutes: number,
): Detection[] {
  const cutoff = now - staleMinutes * 60_000;
  return works.flatMap((work): Detection[] => {
    if (work.state !== "active" || !work.heldBy) return [];
    const holder = sessions.get(work.heldBy);
    if (!holder?.live || Date.parse(holder.lastSeen) >= cutoff) return [];
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
        smallestAction: `maestro work show ${work.id}`,
      }),
      subjectSession: work.heldBy,
      subjectWork: work.id,
    }];
  });
}

function repeatedFailureDetections(
  context: PluginContext,
  works: AttentionWorkRow[],
): Detection[] {
  const dispatch = context.dispatch as DispatchService;
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
    const holderRole = work.heldBy && dispatch
        .list(work.id)
        .some((record) => record.state === "open" && record.heldBy === work.heldBy)
      ? "peer"
      : "lead";
    const route = holderRole === "peer" ? "lead" : "supervisor";
    const startEvidence = start ? `work.start #${start.id}` : "no work.start on record";
    return [{
      entityId: work.id,
      entityType: "work",
      fingerprint: `repeat:${work.id}:${third.id}`,
      holderRole,
      kind: "REPEATED_FAILURE",
      packet: packet("REPEATED_FAILURE", work.id, {
        observed: `${notes.length} failed passes since the latest lease`,
        holderRole,
        evidence: `${startEvidence}; third failed note #${third.id}`,
        unknown: "whether the failures share one mechanism or need a new decision",
        question: "inspect the episode, re-scope, or revisit the decision?",
        smallestAction: `maestro work show ${work.id}`,
      }),
      route,
      subjectSession: work.heldBy,
      subjectWork: work.id,
    }];
  });
}

function decisionStaleDetections(
  context: PluginContext,
  workById: Map<string, AttentionWorkRow>,
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
    }];
  });
}

function humanDecisionRequiredDetections(
  context: PluginContext,
  workById: Map<string, AttentionWorkRow>,
): Detection[] {
  const rows = context.store.database
    .query<{ id: string; work_id: string }, []>(
      `SELECT decisions.id, decisions.work_id
       FROM decisions
       JOIN work ON work.id = decisions.work_id
       WHERE decisions.state = 'draft'
         AND decisions.needs_owner = 1
         AND work.state != 'done'
         AND work.cancelled_at IS NULL
       ORDER BY decisions.id`,
    )
    .all();
  return rows.flatMap((decision): Detection[] => {
    const work = workById.get(decision.work_id);
    if (!work) return [];
    return [{
      entityId: decision.id,
      entityType: "decision",
      fingerprint: `human-decision:${decision.id}`,
      kind: "HUMAN_DECISION_REQUIRED",
      packet: packet("HUMAN_DECISION_REQUIRED", decision.id, {
        observed: `draft linked to ${decision.work_id} requires an owner decision`,
        evidence: "decisions.needs_owner = 1",
        unknown: "which option the owner will choose",
        question: "what should the owner decide?",
        smallestAction: `maestro decision show ${decision.id}`,
        humanDecision: true,
      }),
      subjectSession: work.heldBy,
      subjectWork: work.id,
    }];
  });
}

function scopeCollisionDetections(
  works: AttentionWorkRow[],
  sessions: Map<string, SessionRecord>,
): Detection[] {
  const active = works.filter(
    (work) =>
      work.state === "active" && work.parentId && work.heldBy &&
      sessions.get(work.heldBy)?.live,
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
      });
    }
  }
  return detections;
}

function leadCollisionDetections(
  context: PluginContext,
  works: AttentionWorkRow[],
  sessions: Map<string, SessionRecord>,
): Detection[] {
  const deliveryPeers = new Set(
    (context.dispatch as DispatchService)
      .list()
      .filter(
        (record) =>
          record.state === "open" && record.lane === "delivery" && record.heldBy,
      )
      .map((record) => `${record.workId}:${record.heldBy}`),
  );
  const active = works
    .filter(
      (work) =>
        work.state === "active" && !work.parentId && work.heldBy &&
        sessions.get(work.heldBy)?.live &&
        !deliveryPeers.has(`${work.id}:${work.heldBy}`),
    )
    .sort((left, right) => left.id.localeCompare(right.id));
  const detections: Detection[] = [];
  for (let leftIndex = 0; leftIndex < active.length; leftIndex += 1) {
    const first = active[leftIndex] as AttentionWorkRow;
    for (let rightIndex = leftIndex + 1; rightIndex < active.length; rightIndex += 1) {
      const second = active[rightIndex] as AttentionWorkRow;
      if (first.heldBy === second.heldBy) continue;
      const holders = [first.heldBy as string, second.heldBy as string].sort();
      detections.push({
        entityId: first.id,
        entityType: "work",
        fingerprint:
          `lead-collision:${first.id}:${second.id}:${holders[0]}:${holders[1]}`,
        kind: "LEAD_COLLISION",
        packet: packet("LEAD_COLLISION", `${first.id},${second.id}`, {
          observed:
            `parent work ${first.id} held by ${first.heldBy}; ` +
            `parent work ${second.id} held by ${second.heldBy}`,
          evidence:
            "both work items active and parentless; both sessions live; " +
            "neither holder has an open delivery dispatch on its work item",
          unknown:
            "whether the work items are independent project dimensions or competing Lead scopes",
          question: "keep both Leads, split scope, or release one lease?",
          smallestAction: "maestro status --live",
        }),
        subjectSession: holders.join(","),
        subjectWork: first.id,
      });
    }
  }
  return detections;
}

function dispatchUnreturnedDetections(
  context: PluginContext,
  workById: Map<string, AttentionWorkRow>,
  sessions: Map<string, SessionRecord>,
  now: number,
  dispatchStaleHours: number,
): Detection[] {
  const cutoff = now - dispatchStaleHours * 60 * 60_000;
  const dispatch = context.dispatch as DispatchService;
  return dispatch.list().flatMap((record): Detection[] => {
    if (record.state !== "open") return [];
    const work = workById.get(record.workId);
    if (!work || work.state === "done" || work.state === "cancelled") return [];
    const holder = record.heldBy ? sessions.get(record.heldBy) : undefined;
    if ((!holder || holder.live) && Date.parse(record.createdAt) >= cutoff) return [];
    const subjectSession = record.heldBy ?? record.targetSession;
    const observed = holder
      ? holder.live
        ? `dispatch for ${record.workId} has no handback after ${minutesSince(record.createdAt, now)} minutes; holder session ${holder.id} is live`
        : `dispatch for ${record.workId} has no handback; holder session ${holder.id} is dead`
      : `dispatch for ${record.workId} has no handback after ${minutesSince(record.createdAt, now)} minutes`;
    const evidence = holder
      ? `dispatches.held_by ${holder.id}; sessions.live ${holder.live}; no handbacks row`
      : `dispatches.created_at ${record.createdAt}; no handbacks row`;
    const unknown = holder
      ? holder.live
        ? "whether the live lane is working, blocked, or needs more time"
        : "why the dead holder stopped without filing a handback"
      : "whether the lane is working, blocked, or abandoned";
    return [{
      entityId: record.id,
      entityType: "dispatch",
      fingerprint: `dispatch-unreturned:${record.id}`,
      kind: "DISPATCH_UNRETURNED",
      packet: packet("DISPATCH_UNRETURNED", record.id, {
        observed,
        evidence,
        unknown,
        question: "wait, contact the lane, cancel it, or re-scope?",
        smallestAction: `maestro dispatch show ${record.id}`,
      }),
      subjectSession,
      subjectWork: record.workId,
    }];
  });
}

function dispatchUnacceptedDetections(
  context: PluginContext,
  workById: Map<string, AttentionWorkRow>,
  now: number,
): Detection[] {
  const cutoff = now - 10 * 60_000;
  const dispatch = context.dispatch as DispatchService;
  return dispatch.list().flatMap((record): Detection[] => {
    if (
      record.state !== "open" ||
      record.heldBy !== null ||
      Date.parse(record.createdAt) > cutoff
    ) {
      return [];
    }
    const work = workById.get(record.workId);
    if (!work || work.state === "done" || work.state === "cancelled") return [];
    const pane = record.pane ?? "none";
    return [{
      entityId: record.id,
      entityType: "dispatch",
      fingerprint: `dispatch-unaccepted:${record.id}`,
      kind: "DISPATCH_UNACCEPTED",
      packet: packet("DISPATCH_UNACCEPTED", record.id, {
        observed:
          `${record.id} opened ${minutesSince(record.createdAt, now)} minutes ago ` +
          `on pane ${pane}, never accepted`,
        evidence: "dispatch state open; no session bound to the pane",
        unknown: "whether the brief reached the pane",
        question: "was the stored contract delivered?",
        smallestAction:
          `herdr agent list, then herdr agent prompt <name> with the stored contract from ` +
          `maestro dispatch show ${record.id}`,
      }),
      subjectSession: record.targetSession,
      subjectWork: record.workId,
    }];
  });
}

function handbackUnreviewedDetections(
  context: PluginContext,
  workById: Map<string, AttentionWorkRow>,
  now: number,
): Detection[] {
  const dispatch = context.dispatch as DispatchService;
  const handbacks = context.handback as HandbackService;
  const dispatches = dispatch.list();
  return dispatches.flatMap((record): Detection[] => {
    if (record.state !== "returned") return [];
    const work = workById.get(record.workId);
    if (!work || work.state === "done" || work.state === "cancelled") return [];
    const latest = handbacks.list(record.id).at(-1);
    if (!latest) return [];
    // A sealed council's packets are unreadable by design; the finding
    // appears once the seal opens.
    if (dispatch.council(record.workId, record.id).sealed) return [];
    const cites = new RegExp(`\\b${latest.id}\\b`);
    const reviewed = dispatches.some(
      (other) =>
        other.workId === record.workId && other.createdAt > latest.createdAt &&
        (cites.test(other.objective) || cites.test(other.evidenceRequired)),
    );
    if (reviewed) return [];
    let question = "close the work, re-dispatch, or cancel?";
    let smallestAction = `maestro handback show ${latest.id}`;
    if (latest.status === "BLOCKED") {
      question = "retry condition met?";
      smallestAction = "re-dispatch on the same pane or cancel";
    } else if (latest.status === "DEPENDENCY_REQUEST") {
      question = "accept or decline the dependency request?";
      smallestAction = "open the dependency as a work item in the other scope";
    } else if (latest.status === "COUNCIL_REQUEST") {
      question = "open another council generation or decline?";
      smallestAction = "open a second generation (d688) or decline with a work note";
    } else if (latest.status === "REOPEN_REQUEST") {
      question = "grant another lease or decline?";
      smallestAction = "grant a new lease or decline";
    }
    return [{
      entityId: record.id,
      entityType: "dispatch",
      fingerprint: `handback-unreviewed:${record.id}`,
      kind: "HANDBACK_UNREVIEWED",
      packet: packet("HANDBACK_UNREVIEWED", record.id, {
        observed:
          `${record.id} returned ${latest.status} (${latest.id}) ${minutesSince(latest.createdAt, now)} minutes ago; work ${record.workId} is still ${work.state}`,
        evidence: `handbacks row ${latest.id} status ${latest.status}; work.state ${work.state}; no later dispatch on ${record.workId} cites ${latest.id}`,
        unknown: "whether the Lead has read the return packet",
        question,
        smallestAction,
      }),
      subjectSession: record.heldBy ?? record.targetSession,
      subjectWork: record.workId,
    }];
  });
}

function detect(context: PluginContext, options: AttentionOptions): Detection[] {
  const now = Date.now();
  const works = workRows(context);
  const workById = new Map(works.map((work) => [work.id, work]));
  const sessions = sessionMap(context.sessions.list());
  return [
    ...stalledDetections(
      context,
      works,
      sessions,
      now,
      options.staleMinutes,
    ),
    ...repeatedFailureDetections(context, works),
    ...decisionStaleDetections(
      context,
      workById,
      now,
      options.decisionStaleHours,
    ),
    ...humanDecisionRequiredDetections(context, workById),
    ...leadCollisionDetections(context, works, sessions),
    ...scopeCollisionDetections(works, sessions),
    ...dispatchUnreturnedDetections(
      context,
      workById,
      sessions,
      now,
      options.dispatchStaleHours,
    ),
    ...dispatchUnacceptedDetections(context, workById, now),
    ...handbackUnreviewedDetections(context, workById, now),
  ];
}

function raise(context: PluginContext, detection: Detection): AttentionFinding {
  if (context.store.readOnly) {
    return {
      fingerprint: detection.fingerprint,
      holderRole: detection.holderRole,
      kind: detection.kind,
      packet: detection.packet,
      raised: false,
      raisedAt: "not recorded (read-only)",
      route: detection.route,
      subjectSession: detection.subjectSession,
      subjectWork: detection.subjectWork,
    };
  }
  const createdAt = new Date().toISOString();
  const transaction = context.store.database.transaction(() => {
    const existing = context.store.database
      .query<AttentionRow, [string]>(
        "SELECT created_at FROM attention WHERE fingerprint = ?",
      )
      .get(detection.fingerprint);
    if (existing) return { raised: false, raisedAt: existing.created_at };
    context.store.database
      .query(
        `INSERT INTO attention
          (kind, fingerprint, subject_work, subject_session, packet, created_at)
         VALUES (?, ?, ?, ?, ?, ?)`,
      )
      .run(
        detection.kind,
        detection.fingerprint,
        detection.subjectWork,
        detection.subjectSession,
        detection.packet,
        createdAt,
      );
    context.log.append({
      type: "attention.raise",
      entityType: detection.entityType,
      entityId: detection.entityId,
      sessionId: context.sessions.current().id,
      payload: {
        fingerprint: detection.fingerprint,
        kind: detection.kind,
      },
    });
    return { raised: true, raisedAt: createdAt };
  });
  const result = transaction.immediate();
  return {
    fingerprint: detection.fingerprint,
    holderRole: detection.holderRole,
    kind: detection.kind,
    packet: detection.packet,
    raised: result.raised,
    raisedAt: result.raisedAt,
    route: detection.route,
    subjectSession: detection.subjectSession,
    subjectWork: detection.subjectWork,
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

export const attentionPlugin: BuiltInPlugin = {
  name: "attention",
  inject: ["work", "dispatch"],
  apply(context) {
    context.store.migrate(`
      CREATE TABLE IF NOT EXISTS attention (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        kind TEXT NOT NULL,
        fingerprint TEXT NOT NULL UNIQUE,
        subject_work TEXT,
        subject_session TEXT,
        packet TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
    `);
    const service: AttentionService = {
      scan: (options) => detect(context, options).map((detection) => raise(context, detection)),
    };
    context.effect(() => context.provide("attention", service));
    context.effect(() =>
      (context.brief as BriefService).register(() =>
        service
          .scan({ staleMinutes: 30, decisionStaleHours: 24, dispatchStaleHours: 2 })
          .filter((finding) => finding.kind !== "REPEATED_FAILURE" || finding.route === "lead")
          .map((finding) => finding.packet.split("\n")[0] ?? finding.kind)
          .join("\n")
      )
    );
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
          mutates: false,
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
          mutates: false,
        },
      ),
    );
  },
};
