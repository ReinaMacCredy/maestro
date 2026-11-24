import type { CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { BriefService } from "./coordination.ts";
import type { WorkService } from "./work.ts";

export type AttentionKind =
  | "REPEATED_FAILURE"
  | "HUMAN_DECISION_REQUIRED"
  | "LESSONS_PENDING";

interface AttentionWorkRow {
  heldBy: string | null;
  id: string;
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
  entityType: "decision" | "lesson" | "work";
  fingerprint: string;
  kind: AttentionKind;
  packet: string;
  subjectSession: string | null;
  subjectWork: string | null;
}

export interface AttentionFinding {
  fingerprint: string;
  kind: AttentionKind;
  packet: string;
  raised: boolean;
  raisedAt: string;
  subjectSession: string | null;
  subjectWork: string | null;
}

export interface AttentionService {
  scan(): AttentionFinding[];
}

const subjectKind: Record<AttentionKind, "decision" | "lesson" | "work"> = {
  REPEATED_FAILURE: "work",
  HUMAN_DECISION_REQUIRED: "decision",
  LESSONS_PENDING: "lesson",
};

function workRows(context: PluginContext): AttentionWorkRow[] {
  return (context.work as WorkService)
    .snapshot()
    .map((work) => ({
      heldBy: work.heldBy,
      id: work.id,
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

function packet(
  kind: AttentionKind,
  subject: string,
  fields: {
    evidence: string;
    // room d129 point 2: one id is not enough to act on. A kind may replace the
    // "<subject kind> <subject>" tail of its headline with what the reader
    // needs, and the packet fields below stay as they are.
    headline?: string;
    humanDecision?: boolean;
    observed: string;
    question: string;
    smallestAction: string;
    unknown: string;
  },
): string {
  return [
    `attention ${kind} ${fields.headline ?? `${subjectKind[kind]} ${subject}`}`,
    `  observed: ${fields.observed}`,
    `  evidence: ${fields.evidence}`,
    `  unknown: ${fields.unknown}`,
    `  question: ${fields.question}`,
    `  smallest action: ${fields.smallestAction}`,
    `  human decision needed: ${fields.humanDecision ? "yes" : "no"}`,
  ].join("\n");
}

function repeatedFailureDetections(
  context: PluginContext,
  works: AttentionWorkRow[],
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
    }];
  });
}

function humanDecisionRequiredDetections(
  context: PluginContext,
  workById: Map<string, AttentionWorkRow>,
): Detection[] {
  if (!context.store.hasColumn("decisions", "needs_owner")) return [];
  const activePredicate = context.store.hasColumn("decisions", "withdrawn_at")
    ? "AND decisions.withdrawn_at IS NULL"
    : "";
  const rows = context.store.database
    .query<{ id: string; work_id: string }, []>(
      `SELECT decisions.id, decisions.work_id
       FROM decisions
       JOIN work ON work.id = decisions.work_id
       WHERE decisions.state = 'draft'
         ${activePredicate}
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

function lessonsPendingDetections(context: PluginContext, now: number): Detection[] {
  if (!context.store.hasColumn("lessons", "state")) return [];
  const pending = context.store.database
    .query<{ created_at: string; id: string }, []>(
      `SELECT id, created_at FROM lessons
       WHERE state = 'pending'
       ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)`,
    )
    .all();
  const oldest = pending[0];
  if (!oldest) return [];
  const age = Math.max(0, Math.floor((now - Date.parse(oldest.created_at)) / (24 * 60 * 60_000)));
  const days = age === 1 ? "1 day" : `${age} days`;
  const count = pending.length === 1 ? "1 lesson" : `${pending.length} lessons`;
  return [{
    entityId: oldest.id,
    entityType: "lesson",
    fingerprint: `lessons:${oldest.id}`,
    kind: "LESSONS_PENDING",
    packet: packet("LESSONS_PENDING", oldest.id, {
      headline: `${count} pending, oldest ${oldest.id} (${days}); read them: maestro lesson list`,
      observed: `${count} pending, oldest filed ${oldest.created_at}`,
      evidence: `lessons.state = 'pending'; ${oldest.id} lessons.created_at ${oldest.created_at}`,
      unknown: "whether these corrections still describe the doctrine as it stands",
      question: "edit the doctrine they name, or answer them with maestro lesson process?",
      smallestAction: "maestro lesson list",
    }),
    subjectSession: null,
    subjectWork: null,
  }];
}

function detect(context: PluginContext): Detection[] {
  const works = workRows(context);
  const workById = new Map(works.map((work) => [work.id, work]));
  return [
    ...repeatedFailureDetections(context, works),
    ...humanDecisionRequiredDetections(context, workById),
    ...lessonsPendingDetections(context, Date.now()),
  ];
}

function raise(context: PluginContext, detection: Detection): AttentionFinding {
  if (context.store.readOnly) {
    return {
      fingerprint: detection.fingerprint,
      kind: detection.kind,
      packet: detection.packet,
      raised: false,
      raisedAt: "not recorded (read-only)",
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
    kind: detection.kind,
    packet: detection.packet,
    raised: result.raised,
    raisedAt: result.raisedAt,
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

export const attentionPlugin: BuiltInPlugin = {
  name: "attention",
  inject: ["work"],
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
      scan: () => detect(context).map((detection) => raise(context, detection)),
    };
    context.effect(() => context.provide("attention", service));
    context.effect(() =>
      (context.brief as BriefService).register((_sessionId, event) =>
        service
          .scan()
          .filter((finding) => event === "SessionStart" || finding.kind !== "LESSONS_PENDING")
          .map((finding) => finding.packet.split("\n")[0] ?? finding.kind)
          .join("\n")
      )
    );
    context.effect(() =>
      context.cli.register(
        "attention",
        (): CliResult => {
          const findings = service.scan();
          return { data: { detections: findings }, text: formatFindings(findings) };
        },
        {
          description: "Scan store state for attention packets without mutating work.",
          mutates: false,
          rootDescription: "Raise one-shot attention packets from store evidence.",
        },
      ),
    );
  },
};
