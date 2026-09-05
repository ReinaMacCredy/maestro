import {
  CliError,
  requiredPosition,
  stringOption,
  stringOptions,
  type CliInvocation,
  type CliResult,
} from "../kernel/cli.ts";
import { existsSync, readFileSync } from "node:fs";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { DecisionService } from "./decision.ts";
import type { DispatchService } from "./dispatch.ts";
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
      : "maestro status";
  }
  return unresolvedBlockers(context, blocker.id).length === 0
    ? `maestro work start ${blocker.id}`
    : "maestro ready";
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
          const slp = await maybeHandleSlpWorkAdd(context, invocation);
          if (slp) return slp;
          const title = requiredPosition(invocation, 0, "work title");
          const parentId = stringOption(invocation, "parent") ?? null;
          const blockers = stringOptions(invocation, "blocked-by");
          for (const blocker of blockers) requireWork(context, blocker);
          let id = "";
          const now = new Date().toISOString();
          const kind = stringOption(invocation, "kind") ?? "task";
          const acceptance = stringOption(invocation, "acceptance") ?? null;
          const atomicReason = stringOption(invocation, "atomic-reason") ?? null;
          const gate = await context.events.waterfall<WorkAddGateInput, GateResult>(
            "work.add",
            { kind, parentId, sessionId: context.sessions.current().id, title },
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
                  (id, title, kind, state, parent_id, acceptance, atomic_reason, created_at, updated_at)
                 VALUES (?, ?, ?, 'open', ?, ?, ?, ?, ?)`,
              )
              .run(id, title, kind, parentId, acceptance, atomicReason, now, now);
            const insertBlocker = context.store.database.query(
              "INSERT INTO work_blockers (work_id, blocker_id) VALUES (?, ?)",
            );
            for (const blocker of blockers) insertBlocker.run(id, blocker);
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
            sessionId: context.sessions.current().id,
            payload: { title, kind, parentId, acceptance, atomicReason, blockers },
          });
          return { data: { work: service.get(id) }, text: `${id} added: ${title}` };
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
            "--atomic-reason": {
              description: "Explain why parentless work needs no child breakdown.",
              value: true,
            },
            "--to": {
              description: "Assign SLP work to a Peer; Team Supervisor work defaults to Lead.",
              value: true,
            },
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
        // Declaring an existing item atomic here is the only way out of a
        // breakdown gate without filing a duplicate work item.
        const declaredAtomic = stringOption(invocation, "atomic-reason");
        const gateWork = declaredAtomic ? { ...work, atomicReason: declaredAtomic } : work;
        const children = service.children(id);
        const result = await context.events.waterfall<
          WorkGateInput,
          GateResult
        >(
          "work.start",
          { work: { ...gateWork, heldBy: null }, children, sessionId: session.id },
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
          if (declaredAtomic) {
            context.store.database
              .query("UPDATE work SET atomic_reason = ?, updated_at = ? WHERE id = ?")
              .run(declaredAtomic, now, id);
          }
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
        flags: {
          "--atomic-reason": {
            description: "Declare this item atomic instead of breaking it down.",
            value: true,
          },
        },
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
        if (work.heldBy !== sessionId) {
          if (work.heldBy) {
            throw new CliError("LEASE_HELD", `${id} is held by ${work.heldBy}`, {
              holder: work.heldBy,
            });
          }
          throw new CliError("LEASE_REQUIRED", `${id} has no lease to release`, { id });
        }
        const updatedAt = new Date().toISOString();
        context.store.database.transaction(() => {
          if (!service.release(id, sessionId, updatedAt)) {
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
            payload: { holder: sessionId },
          });
        })();
        return { data: { work: service.get(id) }, text: `${id} released by ${sessionId}` };
      }, {
        description: "Release the current session's lease without completing work.",
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
        if (invocation.options.stall !== undefined) {
          throw new CliError("INVALID_OPTION", "--stall is available only for SLP work");
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
          "--rework": {
            description: "Grant the current SLP return revision one reviewer-authorized retake.",
          },
          "--stall": {
            description: "Observer only: record a stall (repeat|silence|dialog) and nudge the stuck seat.",
            value: true,
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
          const declaredAtomic = stringOption(invocation, "atomic-reason") ?? null;
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
                work: { ...work, heldBy: null, atomicReason: declaredAtomic ?? work.atomicReason },
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
              work: { ...work, atomicReason: declaredAtomic ?? work.atomicReason },
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
                   SET state = 'active', held_by = ?, atomic_reason = COALESCE(?, atomic_reason), updated_at = ?
                   WHERE id = ? AND state != 'done' AND cancelled_at IS NULL AND held_by IS NULL`,
                )
                .run(sessionId, declaredAtomic, now, id);
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
                 SET state = 'done', evidence = ?, candidate = ?, atomic_reason = COALESCE(?, atomic_reason), held_by = NULL, updated_at = ?
                 WHERE id = ? AND state = 'active' AND cancelled_at IS NULL AND held_by = ?`,
              )
              .run(recordedEvidence, candidate, declaredAtomic, now, id, sessionId);
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
                ...(declaredAtomic ? { atomicReason: declaredAtomic } : {}),
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
          return { data: { work: completed }, text: `${id} done${lostLease}${unlanded}` };
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
            "--atomic-reason": {
              description: "Declare an unheld parentless item atomic while done takes its lease.",
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
        const dispatches = (context.dispatch as DispatchService | undefined)?.list(work.id) ?? [];
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
          data: { work, children, blockers, notes, noteCount: allNotes.length, dispatches, decisions, gates },
          text: [
            formatWork(work),
            ...gates.map((gate) => `gate: ${gate.policy} ${gate.requires}`),
            ...blockerLines,
            ...childLines,
            ...(noteCountLine ? [noteCountLine] : []),
            ...notes.map((note) => `note: ${note.text}`),
            ...dispatches.map(
              (dispatch) =>
                `dispatch: ${dispatch.id} [${dispatch.state}] ${dispatch.lane}: ${dispatch.objective}`,
            ),
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
        (): CliResult => {
          const works = service.list();
          const known = new Set(works.map((work) => work.id));
          const childrenByParent = new Map<string, WorkRecord[]>();
          for (const work of works) {
            if (!work.parentId || !known.has(work.parentId)) continue;
            const siblings = childrenByParent.get(work.parentId) ?? [];
            siblings.push(work);
            childrenByParent.set(work.parentId, siblings);
          }
          const renderTree = (work: WorkRecord, depth: number): string[] => [
            `${"  ".repeat(depth)}${work.id} [${work.state}] ${work.title}`,
            ...(childrenByParent.get(work.id) ?? []).flatMap((child) =>
              renderTree(child, depth + 1)
            ),
          ];
          const roots = works.filter(
            (work) => !work.parentId || !known.has(work.parentId),
          );
          return {
            data: { works },
            text: roots.flatMap((work) => renderTree(work, 0)).join("\n"),
          };
        },
        { description: "List tracked work and current states.", mutates: false },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "ready",
        async (invocation): Promise<CliResult> => {
          const showBlocked = invocation.options.all === true;
          const allWorks = service.list();
          const childrenByParent = new Map<string, WorkRecord[]>();
          for (const work of allWorks) {
            if (!work.parentId) continue;
            const children = childrenByParent.get(work.parentId) ?? [];
            children.push(work);
            childrenByParent.set(work.parentId, children);
          }
          const items = allWorks.map((work) => {
            const blockers = blockersFor(context, work.id).map((blocker) => ({
              heldBy: blocker.heldBy,
              id: blocker.id,
              state: blocker.state,
            }));
            return { ...work, blockers };
          });
          const candidates = context.ready.project(items);
          const works: typeof items = [];
          const gated: Array<{
            blockers: Array<{ id: string; state: string }>;
            command?: string;
            id: string;
            origin: string;
            reason: string;
            title: string;
          }> = [];
          const sessionId = context.sessions.current().id;
          for (const work of candidates) {
            const result = await context.events.waterfall<
              WorkGateInput,
              GateResult
            >(
              "work.ready",
              { work, children: childrenByParent.get(work.id) ?? [], sessionId },
              async () => ({ blocked: false }),
            );
            if (result.blocked) {
              const details = gateDetails(result);
              gated.push({
                blockers: result.blockers ?? work.blockers.map((blocker) => ({
                  id: blocker.id,
                  state: blocker.state,
                })),
                ...(result.command ? { command: result.command } : {}),
                id: work.id,
                title: work.title,
                ...details,
              });
            } else {
              works.push(work);
            }
          }
          for (const work of items) {
            if (work.state !== "open") continue;
            const blockers = work.blockers.filter(
              (blocker) => blocker.state !== "done" && blocker.state !== "cancelled",
            );
            if (blockers.length === 0) continue;
            const fullBlockers = blockers.map((blocker) => requireWork(context, blocker.id));
            const details = blockerDetails(context, work.id, fullBlockers, sessionId);
            gated.push({
              id: work.id,
              title: work.title,
              origin: "work-blockers",
              reason: details.reason,
              blockers: blockers.map((blocker) => ({ id: blocker.id, state: blocker.state })),
              command: details.command,
            });
          }
          const listed = showBlocked
            ? gated
            : gated.filter((work) => work.origin !== "work-blockers");
          const hiddenBlocked = gated.length - listed.length;
          const lines = [
            ...works.map((work) => `${work.id} ${work.title}`),
            ...listed.map(
              (work) => `${work.id} ${work.title} [gated by ${work.origin}: ${work.reason}]`,
            ),
          ];
          const dispatches = ((context.dispatch as DispatchService | undefined)?.list() ?? [])
            .filter(
              (dispatch) =>
                dispatch.state === "open" &&
                dispatch.heldBy === null &&
                (dispatch.targetSession === null || dispatch.targetSession === sessionId),
            );
          lines.push(
            ...dispatches.map(
              (dispatch) =>
                `dispatch: ${dispatch.id} [takeable] ${dispatch.lane} for ${dispatch.workId}: ${dispatch.objective}`,
            ),
          );
          if (hiddenBlocked > 0) {
            lines.push(`${hiddenBlocked} blocked by open work hidden; --all to list`);
          }
          const held = allWorks.filter(
            (work) => work.state === "active" && work.heldBy === sessionId,
          );
          const allTerminal = allWorks.length > 0 && allWorks.every(
            (work) => work.state === "done" || work.state === "cancelled",
          );
          return {
            data: { works, gated, dispatches },
            text: lines.length > 0
              ? lines.join("\n")
              : held.length > 0
              ? `no ready work; you hold ${held.map((work) => work.id).join(", ")}; ` +
                `finish it: maestro work done ${held[0]?.id}`
              : allTerminal
              ? "no ready work; all tracked work is closed"
              : allWorks.length === 0
              ? 'no ready work; add some: maestro work add "<title>"'
              : "no ready work; inspect tracked work: maestro work list",
          };
        },
        {
          description: "List ready work and gated items with their blockers.",
          flags: {
            "--all": { description: "List blocked work too; --json always carries it under gated." },
          },
          mutates: false,
        },
      ),
    );
  },
};
