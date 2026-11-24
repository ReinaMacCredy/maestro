import {
  CliError,
  requiredPosition,
  stringOption,
  stringOptions,
  type CliInvocation,
  type CliResult,
} from "../kernel/cli.ts";
import type { Database } from "bun:sqlite";
import { existsSync, readFileSync } from "node:fs";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { DecisionService } from "./decision.ts";
import { modifiedTrackedFiles, namesCommit } from "./git-status.ts";
import { registerSessionCommand } from "./session-required.ts";
import { maybeHandleSlpWorkAdd, maybeHandleSlpWorkNote } from "./slp-v2.ts";

export interface WorkRecord {
  id: string;
  title: string;
  kind: string;
  state: "open" | "active" | "done" | "cancelled";
  parentId: string | null;
  acceptance: string | null;
  atomicReason: string | null;
  evidence: string | null;
  candidate: string | null;
  heldBy: string | null;
  reclaimedFrom: string | null;
  reclaimedBy: string | null;
  reclaimReason: string | null;
  cancelReason: string | null;
  createdAt: string;
  updatedAt: string;
}

interface WorkRow {
  id: string;
  title: string;
  kind: string;
  state: "open" | "active" | "done";
  parent_id: string | null;
  acceptance: string | null;
  atomic_reason: string | null;
  evidence: string | null;
  candidate: string | null;
  held_by: string | null;
  reclaimed_from: string | null;
  reclaimed_by: string | null;
  reclaim_reason: string | null;
  cancelled_at: string | null;
  cancel_reason: string | null;
  created_at: string;
  updated_at: string;
}

interface WorkNoteRow {
  text: string;
  created_at: string;
}

export interface Checkpoint {
  avoid: string;
  next: string;
  state: string;
}

// The latest `checkpoint:` note per work item (Hub d105/d106): three lines,
// state / next / avoid, written incrementally so a compaction never loses them.
export function latestCheckpoints(
  database: Database,
  workIds: string[],
): Map<string, Checkpoint> {
  if (workIds.length === 0) return new Map();
  const placeholders = workIds.map(() => "?").join(", ");
  const rows = database
    .query<{ text: string; work_id: string }, string[]>(
      `SELECT work_id, text FROM work_notes
       WHERE work_id IN (${placeholders}) AND SUBSTR(text, 1, 11) = 'checkpoint:'
       ORDER BY id DESC`,
    )
    .all(...workIds);
  const checkpoints = new Map<string, Checkpoint>();
  for (const row of rows) {
    if (checkpoints.has(row.work_id)) continue;
    checkpoints.set(row.work_id, parseCheckpoint(row.text));
  }
  return checkpoints;
}

function parseCheckpoint(text: string): Checkpoint {
  const fields: Checkpoint = { avoid: "", next: "", state: "" };
  // Agents typing the doc example in a shell send a literal backslash-n; accept both.
  for (const line of text.slice("checkpoint:".length).split(/\n|\\n/)) {
    const match = line.match(/^\s*(state|next|avoid):\s*(.*)$/);
    if (match) fields[match[1] as keyof Checkpoint] = (match[2] ?? "").trim();
  }
  return fields;
}

export interface WorkService {
  get(id: string): WorkRecord | null;
  list(): WorkRecord[];
  children(id: string): WorkRecord[];
  release(id: string, holder: string, updatedAt: string): boolean;
  snapshot(): WorkRecord[];
}

export interface WorkGateInput {
  children: WorkRecord[];
  sessionId: string;
  work: WorkRecord;
}

export interface WorkAddGateInput {
  kind: string;
  parentId: string | null;
  sessionId: string;
  title: string;
}

interface GateResult {
  blocked: boolean;
  blockers?: Array<{ id: string; state: string }>;
  command?: string;
  evidence?: string;
  origin?: string;
  reason?: string;
}

interface ListedWork extends WorkRecord {
  blockedBy: Array<{ id: string; state: string }>;
  gate: { command: string | null; origin: string; reason: string } | null;
  holder: { harness: string | null; id: string; lastSeen: string | null; live: boolean } | null;
}

function toWork(row: WorkRow): WorkRecord {
  return {
    id: row.id,
    title: row.title,
    kind: row.kind,
    state: row.cancelled_at ? "cancelled" : row.state,
    parentId: row.parent_id,
    acceptance: row.acceptance,
    atomicReason: row.atomic_reason,
    evidence: row.evidence,
    candidate: row.candidate ?? null,
    heldBy: row.held_by,
    reclaimedFrom: row.reclaimed_from,
    reclaimedBy: row.reclaimed_by,
    reclaimReason: row.reclaim_reason,
    cancelReason: row.cancel_reason,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

function getWork(context: PluginContext, id: string): WorkRecord | null {
  const row = context.store.database
    .query<WorkRow, [string]>("SELECT * FROM work WHERE id = ?")
    .get(id);
  return row ? expireDeadLease(context, toWork(row)) : null;
}

function expireDeadLease(context: PluginContext, work: WorkRecord): WorkRecord {
  if (!work.heldBy) return work;
  const previousHolder = work.heldBy;
  const liveness = context.sessions.liveness(previousHolder);
  if (liveness.live) return work;
  const updatedAt = new Date().toISOString();
  const expired = { ...work, state: "open" as const, heldBy: null, updatedAt };
  if (context.store.readOnly) return expired;
  const result = context.store.database
    .query(
      `UPDATE work
       SET state = 'open', held_by = NULL, updated_at = ?
       WHERE id = ? AND held_by = ? AND state = 'active'`,
    )
    .run(updatedAt, work.id, previousHolder);
  if (result.changes === 0) return work;
  context.log.append({
    type: "work.lease.expire",
    entityType: "work",
    entityId: work.id,
    payload: { holder: previousHolder, reason: liveness.reason },
  });
  return expired;
}

function latestLeaseExpiration(
  context: PluginContext,
  id: string,
): { holder: string; reason: string } | null {
  const event = context.log
    .list("work", id)
    .filter((candidate) =>
      candidate.type === "work.start" ||
      candidate.type === "work.done" ||
      candidate.type === "work.lease.expire"
    )
    .at(-1);
  if (event?.type !== "work.lease.expire") return null;
  const payload = event.payload as { holder?: unknown; reason?: unknown };
  return typeof payload.holder === "string" && typeof payload.reason === "string"
    ? { holder: payload.holder, reason: payload.reason }
    : null;
}

function requireWork(context: PluginContext, id: string): WorkRecord {
  const work = getWork(context, id);
  if (!work) {
    throw new CliError("NOT_FOUND", `work not found: ${id}; run: maestro work list`, {
      command: "maestro work list",
      id,
    });
  }
  return work;
}

function blockersFor(context: PluginContext, id: string): WorkRecord[] {
  return context.store.database
    .query<{ id: string }, [string]>(
      `SELECT blocker_id AS id
       FROM work_blockers
       WHERE work_id = ?
       ORDER BY CAST(SUBSTR(blocker_id, 2) AS INTEGER)`,
    )
    .all(id)
    .map((blocker) => requireWork(context, blocker.id));
}

function unresolvedBlockers(context: PluginContext, id: string): WorkRecord[] {
  return blockersFor(context, id).filter(
    (blocker) => blocker.state !== "done" && blocker.state !== "cancelled",
  );
}

function nextBlockerCommand(
  context: PluginContext,
  blocker: WorkRecord,
  sessionId: string,
): string {
  if (blocker.state === "active") {
    return blocker.heldBy === sessionId
      ? `maestro work done ${blocker.id}`
      : "maestro work list";
  }
  return unresolvedBlockers(context, blocker.id).length === 0
    ? `maestro work start ${blocker.id}`
    : "maestro work list";
}

function blockerDetails(
  context: PluginContext,
  id: string,
  blockers: WorkRecord[],
  sessionId: string,
): { blockers: string[]; command: string; reason: string } {
  const command = nextBlockerCommand(context, blockers[0] as WorkRecord, sessionId);
  return {
    blockers: blockers.map((blocker) => blocker.id),
    command,
    reason:
      `${id} is blocked by unresolved work: ` +
      `${blockers.map((blocker) => `${blocker.id} [${blocker.state}]`).join(", ")}; ` +
      `run: ${command}`,
  };
}

function listWork(context: PluginContext): WorkRecord[] {
  return context.store.database
    .query<WorkRow, []>("SELECT * FROM work ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)")
    .all()
    .map(toWork)
    .map((work) => expireDeadLease(context, work));
}

function formatWork(work: WorkRecord): string {
  const fields = [
    `${work.id} [${work.state}] ${work.title}`,
    `kind: ${work.kind}`,
    work.parentId ? `parent: ${work.parentId}` : null,
    work.heldBy ? `held by: ${work.heldBy}` : null,
    work.reclaimedFrom ? `reclaimed from: ${work.reclaimedFrom}` : null,
    work.reclaimedBy ? `reclaimed by: ${work.reclaimedBy}` : null,
    work.reclaimReason ? `reclaim reason: ${work.reclaimReason}` : null,
    work.acceptance ? `acceptance: ${work.acceptance}` : null,
    work.atomicReason ? `atomic reason: ${work.atomicReason}` : null,
    work.cancelReason ? `cancel reason: ${work.cancelReason}` : null,
    work.evidence !== null ? `evidence: ${work.evidence}` : null,
    work.candidate !== null ? `candidate: ${work.candidate}` : null,
  ];
  return fields.filter((field): field is string => field !== null).join("\n");
}

function blockIfNeeded(result: GateResult): void {
  if (!result.blocked) return;
  const details = gateDetails(result);
  throw new CliError("GATE_BLOCKED", details.reason, {
    origin: details.origin,
    ...(result.blockers ? { blockers: result.blockers } : {}),
    ...(result.command ? { command: result.command } : {}),
  });
}

function gateDetails(result: GateResult): { origin: string; reason: string } {
  return {
    origin: result.origin ?? "unknown",
    reason: result.reason ?? "gate blocked",
  };
}

export const workPlugin: BuiltInPlugin = {
  name: "work",
  apply(context) {
    context.store.migrate(`
      CREATE TABLE IF NOT EXISTS work (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        kind TEXT NOT NULL,
        state TEXT NOT NULL CHECK(state IN ('open', 'active', 'done')),
        parent_id TEXT REFERENCES work(id),
        acceptance TEXT,
        atomic_reason TEXT,
        evidence TEXT,
        held_by TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS work_blockers (
        work_id TEXT NOT NULL REFERENCES work(id),
        blocker_id TEXT NOT NULL REFERENCES work(id),
        PRIMARY KEY(work_id, blocker_id)
      );
      CREATE TABLE IF NOT EXISTS work_notes (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        work_id TEXT NOT NULL REFERENCES work(id),
        text TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
    `);
    for (const [name, migration] of [
      ["cancelled_at", "ALTER TABLE work ADD COLUMN cancelled_at TEXT"],
      ["cancel_reason", "ALTER TABLE work ADD COLUMN cancel_reason TEXT"],
      ["reclaimed_from", "ALTER TABLE work ADD COLUMN reclaimed_from TEXT"],
      ["reclaimed_by", "ALTER TABLE work ADD COLUMN reclaimed_by TEXT"],
      ["reclaim_reason", "ALTER TABLE work ADD COLUMN reclaim_reason TEXT"],
      ["candidate", "ALTER TABLE work ADD COLUMN candidate TEXT"],
    ] as const) {
      context.store.ensureColumn("work", name, migration);
    }

    const service: WorkService = {
      get: (id) => getWork(context, id),
      list: () => listWork(context),
      children: (id) =>
        context.store.database
          .query<WorkRow, [string]>(
            "SELECT * FROM work WHERE parent_id = ? ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)",
          )
          .all(id)
          .map(toWork)
          .map((work) => expireDeadLease(context, work)),
      release: (id, holder, updatedAt) =>
        context.store.database
          .query(
            "UPDATE work SET state = 'open', held_by = NULL, updated_at = ? WHERE id = ? AND held_by = ?",
          )
          .run(updatedAt, id, holder).changes > 0,
      snapshot: () =>
        context.store.database
          .query<WorkRow, []>("SELECT * FROM work ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)")
          .all()
          .map(toWork),
    };
    context.effect(() => context.provide("work", service));

    context.effect(() =>
      registerSessionCommand(
        context,
        "work add",
        async (invocation): Promise<CliResult> => {
          // room d126: there is no per-item collaboration mode. The flag is
          // declared hidden only so the refusal names the doctrine instead
          // of an unknown-flag error, on every path, before any pane opens.
          if (invocation.options.mode !== undefined) {
            throw new CliError(
              "INVALID_OPTION",
              "--mode is not an option (room d126): items carry no collaboration tag, and a fork is always raised with work note --blocked",
            );
          }
          const start = invocation.options.start === true;
          if (start && invocation.options.to !== undefined) {
            throw new CliError("INVALID_OPTION", "--start is not available for SLP work add --to <peer>; the Peer takes its card with work take");
          }
          const slp = await maybeHandleSlpWorkAdd(context, invocation);
          if (slp) return slp;
          // Hub finding F1 (g7, w734): every option that only a Lead's Peer
          // add can honour refuses the same way here, before any store write.
          // --model and --fresh used to fall through in silence, so an add
          // outside SLP looked as if it had carried them.
          if (invocation.options.profile !== undefined) {
            throw new CliError("INVALID_OPTION", "--profile is available only for SLP work add --to <peer>");
          }
          if (invocation.options.model !== undefined) {
            throw new CliError("INVALID_OPTION", "--model is available only for SLP work add --to <peer>");
          }
          if (invocation.options.fresh !== undefined) {
            throw new CliError("INVALID_OPTION", "--fresh is available only for SLP work add --to <peer>");
          }
          const title = requiredPosition(invocation, 0, "work title");
          const parentId = stringOption(invocation, "parent") ?? null;
          const blockers = stringOptions(invocation, "blocked-by");
          for (const blocker of blockers) requireWork(context, blocker);
          let id = "";
          const now = new Date().toISOString();
          const kind = stringOption(invocation, "kind") ?? "task";
          const acceptance = stringOption(invocation, "acceptance") ?? null;
          const sessionId = context.sessions.current().id;
          const gate = await context.events.waterfall<WorkAddGateInput, GateResult>(
            "work.add",
            { kind, parentId, sessionId, title },
            async () => ({ blocked: false }),
          );
          blockIfNeeded(gate);
          context.store.database.exec("BEGIN IMMEDIATE");
          try {
            if (parentId) {
              const parent = requireWork(context, parentId);
              if (parent.state === "done" || parent.state === "cancelled") {
                throw new CliError(
                  "INVALID_STATE",
                  `${parentId} is ${parent.state} and cannot accept children; add new top-level work instead: maestro work add "<title>"`,
                );
              }
            }
            id = context.store.nextPrefixedId("work", "w");
            context.sessions.record("work.add");
            context.store.database
              .query(
                `INSERT INTO work
                  (id, title, kind, state, parent_id, acceptance, created_at, updated_at)
                 VALUES (?, ?, ?, 'open', ?, ?, ?, ?)`,
              )
              .run(id, title, kind, parentId, acceptance, now, now);
            const insertBlocker = context.store.database.query(
              "INSERT INTO work_blockers (work_id, blocker_id) VALUES (?, ?)",
            );
            for (const blocker of blockers) insertBlocker.run(id, blocker);
            if (start) {
              const unresolved = unresolvedBlockers(context, id);
              if (unresolved.length > 0) {
                const details = blockerDetails(context, id, unresolved, sessionId);
                throw new CliError("BLOCKED", details.reason, {
                  blockers: details.blockers,
                  command: details.command,
                });
              }
              blockIfNeeded(
                await context.events.waterfall<WorkGateInput, GateResult>(
                  "work.start",
                  { work: requireWork(context, id), children: [], sessionId },
                  async () => ({ blocked: false }),
                ),
              );
              context.sessions.record("work.start");
              context.store.database
                .query("UPDATE work SET state = 'active', held_by = ?, updated_at = ? WHERE id = ?")
                .run(sessionId, now, id);
            }
            context.store.database.exec("COMMIT");
          } catch (error) {
            try {
              context.store.database.exec("ROLLBACK");
            } catch {}
            throw error;
          }
          context.log.append({
            type: "work.add",
            entityType: "work",
            entityId: id,
            sessionId,
            payload: { title, kind, parentId, acceptance, blockers },
          });
          if (!start) return { data: { work: service.get(id) }, text: `${id} added: ${title}` };
          context.log.append({
            type: "work.start",
            entityType: "work",
            entityId: id,
            sessionId,
            payload: { holder: sessionId },
          });
          return {
            data: { work: service.get(id) },
            text: `${id} added: ${title}\n${id} started by ${sessionId}`,
          };
        },
        {
          description: "Add a tracked work item.",
          flags: {
            "--kind": {
              description:
                "Set the work kind: feature|task|bug|chore|implement|idea|research (policies key on kind; default task).",
              value: true,
            },
            "--parent": { description: "Attach the item beneath a parent work ID.", value: true },
            "--blocked-by": {
              description: "Add a blocking work ID.",
              value: true,
              multiple: true,
            },
            "--acceptance": { description: "Record the observable acceptance condition.", value: true },
            "--start": { description: "Also start the new item, claiming its lease for this session, as work start does." },
            "--to": {
              description: "Assign SLP work to a Peer; without it the Lead opens an item for itself.",
              value: true,
            },
            "--profile": {
              description: "Lead only: open or reuse the --to Peer with this profile (default: the generation's peer profile, or the profile a --to peer-<name> names).",
              value: true,
            },
            "--model": {
              description: "Lead only: launch the --to Peer with this model on its launch line (default: the generation's --peer-model, else the profile's frontmatter model); passed through unvalidated (room d128), so name a model the profile's harness accepts - peer-challenger is a Codex profile and takes a Codex model, or its pane dies on the harness's own error.",
              value: true,
            },
            "--fresh": {
              description: "Lead only: reset a reused --to Peer pane to a fresh harness context (Claude /clear, Codex /new), proven by a new harness session, and re-check READY before the OPEN push; a no-op on a pane being opened, refused while the Peer holds ACTIVE work or when the reset does not take.",
            },
            "--mode": { hidden: true, value: true },
          },
          positionals: [{ name: "title", required: true }],
          rootDescription: "Manage tracked work, leases, dependencies, and evidence.",
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(context, "work start", async (invocation): Promise<CliResult> => {
        const id = requiredPosition(invocation, 0, "work id");
        const work = requireWork(context, id);
        if (work.state === "done") throw new CliError("INVALID_STATE", `${id} is already done`);
        if (work.state === "cancelled") {
          throw new CliError(
            "INVALID_STATE",
            `${id} is cancelled (${work.cancelReason ?? "no reason recorded"}); add a new work item instead`,
          );
        }
        const session = context.sessions.current();
        if (work.heldBy && work.heldBy !== session.id) {
          throw new CliError("LEASE_HELD", `${id} is held by ${work.heldBy}`, {
            holder: work.heldBy,
          });
        }
        const blockers = unresolvedBlockers(context, id);
        if (blockers.length > 0) {
          const details = blockerDetails(context, id, blockers, session.id);
          throw new CliError("BLOCKED", details.reason, {
            blockers: details.blockers,
            command: details.command,
          });
        }
        const children = service.children(id);
        const result = await context.events.waterfall<
          WorkGateInput,
          GateResult
        >(
          "work.start",
          { work: { ...work, heldBy: null }, children, sessionId: session.id },
          async () => ({ blocked: false }),
        );
        blockIfNeeded(result);
        const claim = context.store.database.transaction(() => {
          const current = requireWork(context, id);
          if (current.state === "done") {
            throw new CliError("INVALID_STATE", `${id} is already done`);
          }
          if (current.state === "cancelled") {
            throw new CliError(
              "INVALID_STATE",
              `${id} is cancelled (${current.cancelReason ?? "no reason recorded"}); add a new work item instead`,
            );
          }
          if (current.heldBy && current.heldBy !== session.id) {
            throw new CliError("LEASE_HELD", `${id} is held by ${current.heldBy}`, {
              holder: current.heldBy,
            });
          }
          const currentBlockers = unresolvedBlockers(context, id);
          if (currentBlockers.length > 0) {
            const details = blockerDetails(context, id, currentBlockers, session.id);
            throw new CliError("BLOCKED", details.reason, {
              blockers: details.blockers,
              command: details.command,
            });
          }
          context.sessions.record("work.start");
          const now = new Date().toISOString();
          const claimed = context.store.database
            .query(
              `UPDATE work
               SET state = 'active', held_by = ?, updated_at = ?
               WHERE id = ? AND state != 'done' AND cancelled_at IS NULL
                 AND (held_by IS NULL OR held_by = ?)`,
            )
            .run(session.id, now, id, session.id);
          if (claimed.changes === 0) {
            const refreshed = requireWork(context, id);
            throw new CliError("LEASE_HELD", `${id} is held by ${refreshed.heldBy}`, {
              holder: refreshed.heldBy,
            });
          }
          context.log.append({
            type: "work.start",
            entityType: "work",
            entityId: id,
            sessionId: session.id,
            payload: { holder: session.id },
          });
          return service.get(id);
        });
        const claimed = claim.immediate();
        return { data: { work: claimed }, text: `${id} started by ${session.id}` };
      }, {
        description: "Start work and claim its live session lease.",
        positionals: [{ name: "id", required: true }],
      }),
    );

    context.effect(() =>
      registerSessionCommand(context, "work release", (invocation): CliResult => {
        const id = requiredPosition(invocation, 0, "work id");
        const work = requireWork(context, id);
        if (work.state === "done" || work.state === "cancelled") {
          throw new CliError("INVALID_STATE", `${id} is ${work.state}; its lease cannot be released`);
        }
        const sessionId = context.sessions.current().id;
        const forced = invocation.options.force === true;
        if (!work.heldBy) {
          throw new CliError("LEASE_REQUIRED", `${id} has no lease to release`, { id });
        }
        if (work.heldBy !== sessionId && !forced) {
          throw new CliError("LEASE_HELD", `${id} is held by ${work.heldBy}`, {
            holder: work.heldBy,
          });
        }
        const holder = work.heldBy;
        const updatedAt = new Date().toISOString();
        context.store.database.transaction(() => {
          if (!service.release(id, holder, updatedAt)) {
            const current = requireWork(context, id);
            throw new CliError("LEASE_HELD", `${id} is held by ${current.heldBy ?? "none"}`, {
              holder: current.heldBy,
            });
          }
          context.log.append({
            type: "work.release",
            entityType: "work",
            entityId: id,
            sessionId,
            payload: holder === sessionId ? { holder } : { holder, forcedBy: sessionId },
          });
        })();
        const text = holder === sessionId
          ? `${id} released by ${sessionId}`
          : `${id} released by ${sessionId}; lease dropped from ${holder}`;
        return { data: { work: service.get(id) }, text };
      }, {
        description: "Release the current session's lease without completing work.",
        flags: {
          "--force": {
            description: "Drop another session's lease even while that session is alive; the event records who forced it.",
          },
        },
        positionals: [{ name: "id", required: true }],
      }),
    );

    context.effect(() =>
      registerSessionCommand(context, "work reclaim", (invocation): CliResult => {
        const id = requiredPosition(invocation, 0, "work id");
        const reason = stringOption(invocation, "reason");
        if (!reason?.trim()) {
          throw new CliError("MISSING_ARGUMENT", "work reclaim requires --reason <text>");
        }
        const work = requireWork(context, id);
        if (work.state === "done" || work.state === "cancelled") {
          throw new CliError("INVALID_STATE", `${id} is ${work.state}; its lease cannot be reclaimed`);
        }
        if (!work.heldBy) {
          const command = `maestro work start ${id}`;
          throw new CliError("LEASE_REQUIRED", `${id} has no lease to reclaim; run: ${command}`, {
            command,
          });
        }
        const previousHolder = work.heldBy;
        const newHolder = context.sessions.current().id;
        const updatedAt = new Date().toISOString();
        context.store.database.transaction(() => {
          context.sessions.record("work.reclaim");
          const result = context.store.database
            .query(
              `UPDATE work
               SET state = 'active', held_by = ?, reclaimed_from = ?, reclaimed_by = ?,
                   reclaim_reason = ?, updated_at = ?
               WHERE id = ? AND held_by = ? AND state = 'active'`,
            )
            .run(newHolder, previousHolder, newHolder, reason, updatedAt, id, previousHolder);
          if (result.changes === 0) {
            const current = requireWork(context, id);
            throw new CliError("LEASE_HELD", `${id} is held by ${current.heldBy ?? "none"}`, {
              holder: current.heldBy,
            });
          }
          context.log.append({
            type: "work.reclaim",
            entityType: "work",
            entityId: id,
            sessionId: newHolder,
            payload: { previousHolder, newHolder, reason },
          });
        })();
        return {
          data: { work: service.get(id) },
          text: `${id} reclaimed by ${newHolder} from ${previousHolder}: ${reason}`,
        };
      }, {
        description: "Take an existing lease with a recorded reason without completing work.",
        flags: {
          "--reason": { description: "Record why this lease is being reclaimed.", value: true },
        },
        positionals: [{ name: "id", required: true }],
      }),
    );

    context.effect(() =>
      registerSessionCommand(context, "work note", async (invocation): Promise<CliResult> => {
        // A long body travels as a file so it never meets the shell's quoting or
        // its command guards; both paths below read the body as positional 1.
        const file = invocation.options.file;
        if (typeof file === "string") {
          if (invocation.positionals[1] !== undefined) {
            throw new CliError("INVALID_OPTION", "--file replaces the note text; give one or the other");
          }
          if (!existsSync(file)) throw new CliError("NOT_FOUND", `note file not found: ${file}`, { file });
          invocation.positionals[1] = readFileSync(file, "utf8").trimEnd();
        }
        const slp = await maybeHandleSlpWorkNote(context, invocation);
        if (slp) return slp;
        if (invocation.options.rework === true) {
          throw new CliError("INVALID_OPTION", "--rework is available only for SLP RETURNED work");
        }
        if (invocation.options.blocked === true) {
          throw new CliError("INVALID_OPTION", "--blocked is available only for SLP work");
        }
        if (invocation.options.owner === true) {
          throw new CliError("INVALID_OPTION", "--owner is available only for SLP work held or reviewed by a seat");
        }
        const id = requiredPosition(invocation, 0, "work id");
        const text = requiredPosition(invocation, 1, "note text");
        requireWork(context, id);
        const createdAt = new Date().toISOString();
        context.store.database
          .query("INSERT INTO work_notes (work_id, text, created_at) VALUES (?, ?, ?)")
          .run(id, text, createdAt);
        context.log.append({
          type: "work.note",
          entityType: "work",
          entityId: id,
          sessionId: context.sessions.current().id,
          payload: { text },
        });
        return { data: { id, text }, text: `${id} note: ${text}` };
      }, {
        description: "Append a note to a work item.",
        flags: {
          "--blocked": {
            description: "Flag the note as blocked and push it one SLP seat up.",
          },
          "--file": {
            description: "Read the note body from this file instead of the text argument.",
            value: true,
          },
          "--owner": {
            description: "Record an owner walk-in typed into this seat's pane (room d123): provenance only, pushed one SLP seat up, never a blocked flag.",
          },
          "--rework": {
            description: "Grant the current SLP return revision one reviewer-authorized retake.",
          },
        },
        positionals: [
          { name: "id", required: true },
          { name: "text", required: true },
        ],
      }),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "work done",
        async (invocation): Promise<CliResult> => {
          const id = requiredPosition(invocation, 0, "work id");
          const work = requireWork(context, id);
          if (work.state === "cancelled") {
            throw new CliError(
              "INVALID_STATE",
              `${id} is cancelled (${work.cancelReason ?? "no reason recorded"}); add a new work item instead`,
            );
          }
          if (work.state === "done") {
            throw new CliError("INVALID_STATE", `${id} is already done; nothing to complete`);
          }
          const sessionId = context.sessions.current().id;
          // A card nobody holds closes in one command: done takes the lease
          // itself. The start gates and blockers run first, so the implicit
          // claim is not a way around anything work start would refuse.
          const claimsLease = work.heldBy !== sessionId;
          const expired = claimsLease ? latestLeaseExpiration(context, id) : null;
          if (claimsLease) {
            if (work.heldBy) {
              throw new CliError("LEASE_HELD", `${id} is held by ${work.heldBy}`, {
                holder: work.heldBy,
              });
            }
            const blockers = unresolvedBlockers(context, id);
            if (blockers.length > 0) {
              const details = blockerDetails(context, id, blockers, sessionId);
              throw new CliError("BLOCKED", details.reason, {
                blockers: details.blockers,
                command: details.command,
              });
            }
            const start = await context.events.waterfall<WorkGateInput, GateResult>(
              "work.start",
              {
                work: { ...work, heldBy: null },
                children: service.children(id),
                sessionId,
              },
              async () => ({ blocked: false }),
            );
            blockIfNeeded(start);
          }
          const evidence = stringOption(invocation, "evidence") ?? "";
          const candidate = stringOption(invocation, "candidate") ?? null;
          const claims = stringOptions(invocation, "claim");
          const proofs = stringOptions(invocation, "proof");
          const result = await context.events.waterfall<
            {
              children: WorkRecord[];
              claims: string[];
              evidence: string;
              proofs: string[];
              sessionId: string;
              work: WorkRecord;
            },
            GateResult
          >(
            "work.done",
            {
              work,
              children: service.children(id),
              evidence,
              claims,
              proofs,
              sessionId,
            },
            async (completion) => ({ blocked: false, evidence: completion.evidence }),
          );
          blockIfNeeded(result);
          const recordedEvidence = result.evidence ?? evidence;
          const complete = context.store.database.transaction(() => {
            const current = requireWork(context, id);
            if (current.state === "cancelled") {
              throw new CliError(
                "INVALID_STATE",
                `${id} is cancelled (${current.cancelReason ?? "no reason recorded"}); add a new work item instead`,
              );
            }
            if (current.state === "done") {
              throw new CliError("INVALID_STATE", `${id} is already done; nothing to complete`);
            }
            const now = new Date().toISOString();
            if (current.heldBy !== sessionId) {
              if (current.heldBy) {
                throw new CliError("LEASE_HELD", `${id} is held by ${current.heldBy}`, {
                  holder: current.heldBy,
                });
              }
              const claimed = context.store.database
                .query(
                  `UPDATE work
                   SET state = 'active', held_by = ?, updated_at = ?
                   WHERE id = ? AND state != 'done' AND cancelled_at IS NULL AND held_by IS NULL`,
                )
                .run(sessionId, now, id);
              if (claimed.changes === 0) {
                const refreshed = requireWork(context, id);
                throw new CliError("LEASE_HELD", `${id} is held by ${refreshed.heldBy}`, {
                  holder: refreshed.heldBy,
                });
              }
            }
            const completed = context.store.database
              .query(
                `UPDATE work
                 SET state = 'done', evidence = ?, candidate = ?, held_by = NULL, updated_at = ?
                 WHERE id = ? AND state = 'active' AND cancelled_at IS NULL AND held_by = ?`,
              )
              .run(recordedEvidence, candidate, now, id, sessionId);
            if (completed.changes === 0) {
              throw new CliError("INVALID_STATE", `${id} changed while completion was pending`);
            }
            context.sessions.record("work.done");
            context.log.append({
              type: "work.done",
              entityType: "work",
              entityId: id,
              sessionId,
              payload: {
                candidate,
                ...(claimsLease ? { claimedOnDone: true } : {}),
                evidence: recordedEvidence,
                claims,
                proofs,
              },
            });
            return service.get(id);
          });
          const completed = complete.immediate();
          const lostLease = expired
            ? `; previous lease held by ${expired.holder} expired because ${expired.reason}`
            : "";
          const modified = candidate || namesCommit([recordedEvidence, ...claims, ...proofs].join(" "))
            ? 0
            : await modifiedTrackedFiles(process.cwd());
          const unlanded = modified > 0
            ? `\nwarning: ${modified} tracked files are modified and the evidence names no commit; a deliverable that only lives in the working tree is not landed`
            : "";
          const reactions = (await context.events.serial("work.completed", completed)).filter((line) => typeof line === "string");
          return { data: { work: completed }, text: [`${id} done${lostLease}${unlanded}`, ...reactions].join("\n") };
        },
        {
          description: "Complete work with policy-checked evidence, taking an unheld lease.",
          flags: {
            "--evidence": {
              description: "Record opaque completion evidence.",
              value: true,
            },
            "--candidate": {
              description: "Record an opaque commit or digest.",
              value: true,
            },
          },
          positionals: [{ name: "id", required: true }],
        },
      ),
    );

    const blockerFlag = {
      "--by": { description: "The blocking work ID.", value: true, multiple: true },
    } as const;
    const blockerIds = (invocation: CliInvocation): string[] => {
      const ids = stringOptions(invocation, "by");
      if (ids.length === 0) {
        throw new CliError("MISSING_ARGUMENT", `${invocation.command} requires --by <work-id>`);
      }
      return ids;
    };
    const editableWork = (id: string): WorkRecord => {
      const work = requireWork(context, id);
      if (work.state === "done" || work.state === "cancelled") {
        throw new CliError("INVALID_STATE", `${id} is ${work.state}; its blockers are history`, {
          id,
          state: work.state,
        });
      }
      return work;
    };
    const dependsOn = (id: string, candidate: string): boolean =>
      context.store.database
        .query<{ id: string }, [string, string]>(
          `WITH RECURSIVE upstream(id) AS (
             SELECT blocker_id FROM work_blockers WHERE work_id = ?
             UNION
             SELECT work_blockers.blocker_id
             FROM work_blockers
             JOIN upstream ON work_blockers.work_id = upstream.id
           )
           SELECT id FROM upstream WHERE id = ?`,
        )
        .get(id, candidate) !== null;

    context.effect(() =>
      registerSessionCommand(
        context,
        "work block",
        (invocation): CliResult => {
          const id = requiredPosition(invocation, 0, "work id");
          const blockers = blockerIds(invocation);
          const edit = context.store.database.transaction(() => {
            editableWork(id);
            const existing = new Set(blockersFor(context, id).map((blocker) => blocker.id));
            for (const blocker of blockers) {
              requireWork(context, blocker);
              if (blocker === id) {
                throw new CliError("INVALID_ARGUMENT", `${id} cannot block itself`, { id });
              }
              if (existing.has(blocker)) {
                throw new CliError("INVALID_STATE", `${id} is already blocked by ${blocker}`, {
                  blocker,
                  id,
                });
              }
              if (dependsOn(blocker, id)) {
                throw new CliError(
                  "INVALID_ARGUMENT",
                  `${blocker} already waits on ${id}; blocking ${id} by ${blocker} would be a cycle`,
                  { blocker, id },
                );
              }
            }
            const insert = context.store.database.query(
              "INSERT INTO work_blockers (work_id, blocker_id) VALUES (?, ?)",
            );
            for (const blocker of blockers) insert.run(id, blocker);
            context.sessions.record("work.block");
            context.log.append({
              type: "work.block",
              entityType: "work",
              entityId: id,
              sessionId: context.sessions.current().id,
              payload: { blockers },
            });
            return blockersFor(context, id);
          });
          const current = edit.immediate();
          return {
            data: { work: service.get(id), blockers: current },
            text: `${id} blocked by ${current.map((blocker) => blocker.id).join(", ")}`,
          };
        },
        {
          description: "Add blocking work IDs to an existing work item.",
          flags: blockerFlag,
          positionals: [{ name: "id", required: true }],
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "work unblock",
        (invocation): CliResult => {
          const id = requiredPosition(invocation, 0, "work id");
          const blockers = blockerIds(invocation);
          const edit = context.store.database.transaction(() => {
            editableWork(id);
            const remove = context.store.database.query(
              "DELETE FROM work_blockers WHERE work_id = ? AND blocker_id = ?",
            );
            for (const blocker of blockers) {
              if (remove.run(id, blocker).changes === 0) {
                throw new CliError("NOT_FOUND", `${id} is not blocked by ${blocker}`, {
                  blocker,
                  id,
                });
              }
            }
            context.sessions.record("work.unblock");
            context.log.append({
              type: "work.unblock",
              entityType: "work",
              entityId: id,
              sessionId: context.sessions.current().id,
              payload: { blockers },
            });
            return blockersFor(context, id);
          });
          const current = edit.immediate();
          return {
            data: { work: service.get(id), blockers: current },
            text: current.length === 0
              ? `${id} unblocked; no blockers left`
              : `${id} unblocked from ${blockers.join(", ")}; still blocked by ${current.map((blocker) => blocker.id).join(", ")}`,
          };
        },
        {
          description: "Remove blocking work IDs from an existing work item.",
          flags: blockerFlag,
          positionals: [{ name: "id", required: true }],
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "work cancel",
        async (invocation): Promise<CliResult> => {
          const id = requiredPosition(invocation, 0, "work id");
          const work = requireWork(context, id);
          if (work.state === "cancelled") {
            throw new CliError("INVALID_STATE", `${id} is already cancelled`);
          }
          if (work.state === "done") {
            throw new CliError("INVALID_STATE", `${id} is done; completed work cannot be cancelled`);
          }
          if (work.state === "active") {
            const sessionId = context.sessions.current().id;
            if (work.heldBy !== sessionId) {
              throw new CliError("LEASE_HELD", `${id} is held by ${work.heldBy}`, {
                holder: work.heldBy,
              });
            }
          }
          const reason = stringOption(invocation, "reason");
          if (!reason) {
            throw new CliError("MISSING_ARGUMENT", "work cancel requires --reason <text>");
          }
          const sessionId = context.sessions.current().id;
          const mutableDescendantsQuery = context.store.database.query<{ id: string }, [string]>(
            `WITH RECURSIVE descendants(id, depth) AS (
               SELECT id, 1 FROM work WHERE parent_id = ?
               UNION ALL
               SELECT work.id, descendants.depth + 1
               FROM work
               JOIN descendants ON work.parent_id = descendants.id
             )
             SELECT work.id
             FROM work
             JOIN descendants ON work.id = descendants.id
             WHERE work.state IN ('open', 'active') AND work.cancelled_at IS NULL
             ORDER BY descendants.depth, CAST(SUBSTR(work.id, 2) AS INTEGER)`,
          );
          const descendantIds = mutableDescendantsQuery.all(id).map((descendant) => descendant.id);
          for (const targetId of [id, ...descendantIds]) {
            const target = targetId === id ? work : requireWork(context, targetId);
            const result = await context.events.waterfall<WorkGateInput, GateResult>(
              "work.cancel",
              { work: target, children: service.children(targetId), sessionId },
              async () => ({ blocked: false }),
            );
            blockIfNeeded(result);
          }
          const cancel = context.store.database.transaction(() => {
            const current = requireWork(context, id);
            if (current.state === "cancelled") {
              throw new CliError("INVALID_STATE", `${id} is already cancelled`);
            }
            if (current.state === "done") {
              throw new CliError("INVALID_STATE", `${id} is done; completed work cannot be cancelled`);
            }
            if (current.state !== work.state || current.heldBy !== work.heldBy) {
              throw new CliError("INVALID_STATE", `${id} changed while cancellation was pending`);
            }
            const now = new Date().toISOString();
            const currentDescendantIds = mutableDescendantsQuery
              .all(id)
              .map((descendant) => descendant.id);
            if (
              currentDescendantIds.length !== descendantIds.length ||
              currentDescendantIds.some((descendantId, index) =>
                descendantId !== descendantIds[index]
              )
            ) {
              throw new CliError("INVALID_STATE", `${id} changed while cancellation was pending`);
            }
            const cancellations = [
              { id, reason },
              ...currentDescendantIds.map((descendantId) => ({
                id: descendantId,
                reason: `parent ${id} cancelled: ${reason}`,
              })),
            ];
            const cancelWork = context.store.database
              .query(
                `UPDATE work
                 SET cancelled_at = ?, cancel_reason = ?, held_by = NULL, updated_at = ?
                 WHERE id = ? AND state IN ('open', 'active') AND cancelled_at IS NULL`,
              );
            for (const cancellation of cancellations) {
              const cancelled = cancelWork.run(
                now,
                cancellation.reason,
                now,
                cancellation.id,
              );
              if (cancelled.changes === 0) {
                throw new CliError(
                  "INVALID_STATE",
                  `${cancellation.id} changed while cancellation was pending`,
                );
              }
            }
            context.sessions.record("work.cancel");
            for (const cancellation of cancellations) {
              context.log.append({
                type: "work.cancel",
                entityType: "work",
                entityId: cancellation.id,
                sessionId,
                payload: { reason: cancellation.reason },
              });
            }
            return service.get(id);
          });
          const cancelled = cancel.immediate();
          return { data: { work: cancelled }, text: `${id} cancelled: ${reason}` };
        },
        {
          description:
            "Cancel open or currently held work and its open or active descendants with a recorded reason.",
          flags: {
            "--reason": { description: "Record why this work is cancelled.", value: true },
          },
          positionals: [{ name: "id", required: true }],
        },
      ),
    );

    context.effect(() =>
      context.cli.register("work show", (invocation): CliResult => {
        const work = requireWork(context, requiredPosition(invocation, 0, "work id"));
        const children = service.children(work.id);
        const blockers = blockersFor(context, work.id);
        const allNotes = context.store.database
          .query<WorkNoteRow, [string]>(
            "SELECT text, created_at FROM work_notes WHERE work_id = ? ORDER BY id",
          )
          .all(work.id)
          .map((note) => ({ text: note.text, createdAt: note.created_at }));
        // The live slice is the last few notes (d821); a relay can leave dozens.
        const window = invocation.options.notes;
        let noteLimit = 5;
        if (window === "all") noteLimit = allNotes.length;
        else if (typeof window === "string") {
          noteLimit = Number(window);
          if (!Number.isInteger(noteLimit) || noteLimit < 0) {
            throw new CliError("INVALID_VALUE", "--notes takes a non-negative integer or all", { notes: window });
          }
        }
        const notes = allNotes.slice(Math.max(0, allNotes.length - noteLimit));
        const noteCountLine = notes.length < allNotes.length
          ? `notes: ${allNotes.length}, showing the last ${notes.length}; maestro work show ${work.id} --notes all`
          : null;
        const childLines = children.map(
          (child) => `child: ${child.id} [${child.state}] ${child.title}`,
        );
        const blockerLines = blockers.map(
          (blocker) => `blocker: ${blocker.id} [${blocker.state}] ${blocker.title}`,
        );
        const decisions = ((context.decision as DecisionService | undefined)?.list() ?? [])
          .filter((decision) => decision.workId === work.id);
        // What done will demand is only discoverable on rejection otherwise:
        // each active policy that gates work done states its requirement here.
        const gates = work.state === "done" || work.state === "cancelled"
          ? []
          : context.loader.records
              .filter(
                (record) =>
                  record.status === "active" &&
                  record.name.startsWith("policy-") &&
                  /^gates [^:]*\bdone\b/.test(record.requires ?? ""),
              )
              .map((record) => ({ policy: record.name, requires: record.requires ?? "" }));
        return {
          data: { work, children, blockers, notes, noteCount: allNotes.length, decisions, gates },
          text: [
            formatWork(work),
            ...gates.map((gate) => `gate: ${gate.policy} ${gate.requires}`),
            ...blockerLines,
            ...childLines,
            ...(noteCountLine ? [noteCountLine] : []),
            ...notes.map((note) => `note: ${note.text}`),
            ...decisions.map(
              (decision) =>
                `decision: ${decision.id} [${decision.state}] ${decision.text}`,
            ),
          ].join("\n"),
        };
      }, {
        description: "Show one work item with its done gates, blockers, evidence, children, and the last five notes.",
        flags: { "--notes": { description: "How many notes to print, newest last: an integer or all (default 5).", value: true } },
        mutates: false,
        positionals: [{ name: "id", required: true }],
      }),
    );

    context.effect(() =>
      context.cli.register(
        "work list",
        async (invocation): Promise<CliResult> => {
          const parentId = stringOption(invocation, "parent");
          const showAll = invocation.options.all === true;
          const database = context.store.database;
          if (parentId && !database.query("SELECT 1 FROM work WHERE id = ?").get(parentId)) {
            throw new CliError("NOT_FOUND", `work not found: ${parentId}; run: maestro work list`, {
              command: "maestro work list",
              id: parentId,
            });
          }
          const order = "ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)";
          const rows = parentId
            ? database.query<WorkRow, [string]>(`SELECT * FROM work WHERE parent_id = ? ${order}`).all(parentId)
            : showAll
            ? database.query<WorkRow, []>(`SELECT * FROM work ${order}`).all()
            : database
                .query<WorkRow, []>(
                  `SELECT * FROM work WHERE state IN ('open', 'active') AND cancelled_at IS NULL ${order}`,
                )
                .all();
          const counts = { open: 0, active: 0, done: 0, cancelled: 0 };
          for (const row of database
            .query<{ count: number; state: keyof typeof counts }, []>(
              `SELECT CASE WHEN cancelled_at IS NOT NULL THEN 'cancelled' ELSE state END AS state,
                      COUNT(*) AS count
               FROM work GROUP BY 1`,
            )
            .all()) counts[row.state] = row.count;

          const blockedBy = new Map<string, Array<{ id: string; state: string }>>();
          for (const edge of database
            .query<{ id: string; state: string; work_id: string }, []>(
              `SELECT work_blockers.work_id, blocker.id, blocker.state
               FROM work_blockers
               JOIN work AS item ON item.id = work_blockers.work_id
               JOIN work AS blocker ON blocker.id = work_blockers.blocker_id
               WHERE item.state != 'done' AND item.cancelled_at IS NULL
                 AND blocker.state != 'done' AND blocker.cancelled_at IS NULL
               ORDER BY CAST(SUBSTR(blocker.id, 2) AS INTEGER)`,
            )
            .all()) {
            blockedBy.set(edge.work_id, [
              ...blockedBy.get(edge.work_id) ?? [],
              { id: edge.id, state: edge.state },
            ]);
          }

          const holders = new Map<string, NonNullable<ListedWork["holder"]>>();
          const holderOf = (id: string): NonNullable<ListedWork["holder"]> => {
            let holder = holders.get(id);
            if (!holder) {
              const session = context.sessions.get(id);
              holder = {
                harness: session?.harness ?? null,
                id,
                lastSeen: session?.lastSeen ?? null,
                live: session?.live ?? false,
              };
              holders.set(id, holder);
            }
            return holder;
          };

          const childrenOf = new Map<string, WorkRecord[]>();
          for (const child of database
            .query<WorkRow, []>(
              `SELECT * FROM work WHERE parent_id IN (
                 SELECT id FROM work WHERE state = 'open' AND cancelled_at IS NULL
               ) ${order}`,
            )
            .all()
            .map(toWork)) {
            childrenOf.set(child.parentId as string, [...childrenOf.get(child.parentId as string) ?? [], child]);
          }
          const sessionId = context.sessions.current().id;
          const works: ListedWork[] = [];
          for (const work of rows.map(toWork)) {
            const blockers = work.state === "open" || work.state === "active"
              ? blockedBy.get(work.id) ?? []
              : [];
            let gate: ListedWork["gate"] = null;
            if (work.state === "open" && !work.heldBy && blockers.length === 0) {
              const result = await context.events.waterfall<WorkGateInput, GateResult>(
                "work.ready",
                { work, children: childrenOf.get(work.id) ?? [], sessionId },
                async () => ({ blocked: false }),
              );
              if (result.blocked) gate = { ...gateDetails(result), command: result.command ?? null };
            }
            works.push({
              ...work,
              blockedBy: blockers,
              holder: work.heldBy ? holderOf(work.heldBy) : null,
              gate,
            });
          }

          const shown = new Set(works.map((work) => work.id));
          const childrenByParent = new Map<string, ListedWork[]>();
          for (const work of works) {
            if (parentId || !work.parentId || !shown.has(work.parentId)) continue;
            childrenByParent.set(work.parentId, [...childrenByParent.get(work.parentId) ?? [], work]);
          }
          const render = (work: ListedWork, depth: number): string[] => {
            const indent = "  ".repeat(depth);
            const held = work.holder
              ? `  held by ${work.holder.id} (${work.holder.live ? "live" : "dead"})`
              : "";
            const blocked = work.blockedBy.length > 0
              ? `  blocked by ${work.blockedBy.map((blocker) => `${blocker.id} [${blocker.state}]`).join(", ")}`
              : "";
            return [
              `${indent}${work.id} [${work.state}] ${work.title}${held}${blocked}`,
              ...(work.gate ? [`${indent}  gated by ${work.gate.origin}: ${work.gate.reason}`] : []),
              ...(childrenByParent.get(work.id) ?? []).flatMap((child) => render(child, depth + 1)),
            ];
          };
          const lines = works
            .filter((work) => parentId || !work.parentId || !shown.has(work.parentId))
            .flatMap((work) => render(work, 0));
          if (!parentId && !showAll) {
            const hidden = [
              counts.done > 0 ? `${counts.done} done` : null,
              counts.cancelled > 0 ? `${counts.cancelled} cancelled` : null,
            ].filter((part): part is string => part !== null);
            if (hidden.length > 0) lines.push(`${hidden.join(", ")} not shown; maestro work list --all`);
          }
          return {
            data: { works, counts },
            text: lines.length > 0
              ? lines.join("\n")
              : parentId
              ? `${parentId} has no children; add one: maestro work add "<title>" --parent ${parentId}`
              : 'no tracked work; add some: maestro work add "<title>"',
          };
        },
        {
          description: "List open and active work with holders, blockers and gates; done and cancelled work is counted.",
          flags: {
            "--all": { description: "List every item, done and cancelled included." },
            "--parent": { description: "List the children of one work ID in every state.", value: true },
          },
          mutates: false,
        },
      ),
    );
  },
};
