import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";

export interface WorkRecord {
  id: string;
  title: string;
  kind: string;
  state: "open" | "active" | "done";
  parentId: string | null;
  acceptance: string | null;
  atomicReason: string | null;
  evidence: string | null;
  heldBy: string | null;
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
  held_by: string | null;
  created_at: string;
  updated_at: string;
}

export interface WorkService {
  get(id: string): WorkRecord | null;
  list(): WorkRecord[];
  children(id: string): WorkRecord[];
}

interface GateResult {
  blocked: boolean;
  evidence?: string;
  origin?: string;
  reason?: string;
}

function toWork(row: WorkRow): WorkRecord {
  return {
    id: row.id,
    title: row.title,
    kind: row.kind,
    state: row.state,
    parentId: row.parent_id,
    acceptance: row.acceptance,
    atomicReason: row.atomic_reason,
    evidence: row.evidence,
    heldBy: row.held_by,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

function textOption(invocation: CliInvocation, name: string): string | undefined {
  const value = invocation.options[name];
  return typeof value === "string" ? value : undefined;
}

function listOption(invocation: CliInvocation, name: string): string[] {
  const value = invocation.options[name];
  if (Array.isArray(value)) return value;
  return typeof value === "string" ? [value] : [];
}

function requirePosition(invocation: CliInvocation, index: number, label: string): string {
  const value = invocation.positionals[index];
  if (!value) throw new CliError("MISSING_ARGUMENT", `missing ${label}`);
  return value;
}

function nextId(context: PluginContext): string {
  const row = context.store.database
    .query<{ next: number }, []>(
      "SELECT COALESCE(MAX(CAST(SUBSTR(id, 2) AS INTEGER)), 0) + 1 AS next FROM work",
    )
    .get();
  return `w${row?.next ?? 1}`;
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
  return { ...work, state: "open", heldBy: null, updatedAt };
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
  if (!work) throw new CliError("NOT_FOUND", `work not found: ${id}`, { id });
  return work;
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
    work.acceptance ? `acceptance: ${work.acceptance}` : null,
    work.atomicReason ? `atomic reason: ${work.atomicReason}` : null,
    work.evidence !== null ? `evidence: ${work.evidence}` : null,
  ];
  return fields.filter((field): field is string => field !== null).join("\n");
}

function blockIfNeeded(result: GateResult): void {
  if (!result.blocked) return;
  throw new CliError("GATE_BLOCKED", result.reason ?? "gate blocked", {
    origin: result.origin ?? "unknown",
    reason: result.reason ?? "gate blocked",
  });
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
    };
    context.effect(() => context.provide("work", service));

    context.effect(() =>
      context.cli.register(
        "work add",
        (invocation): CliResult => {
          const title = requirePosition(invocation, 0, "work title");
          const parentId = textOption(invocation, "parent") ?? null;
          const blockers = listOption(invocation, "blocked-by");
          if (parentId) requireWork(context, parentId);
          for (const blocker of blockers) requireWork(context, blocker);
          let id = "";
          const now = new Date().toISOString();
          const kind = textOption(invocation, "kind") ?? "task";
          const acceptance = textOption(invocation, "acceptance") ?? null;
          const atomicReason = textOption(invocation, "atomic-reason") ?? null;
          context.store.database.exec("BEGIN IMMEDIATE");
          try {
            id = nextId(context);
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
            "--kind": { description: "Set the work kind.", value: true },
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
          },
          maxPositionals: 1,
          rootDescription: "Manage tracked work, leases, dependencies, and evidence.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register("work start", async (invocation): Promise<CliResult> => {
        const id = requirePosition(invocation, 0, "work id");
        const work = requireWork(context, id);
        if (work.state === "done") throw new CliError("INVALID_STATE", `${id} is already done`);
        const session = context.sessions.record("work.start");
        if (work.heldBy && work.heldBy !== session.id) {
          throw new CliError("LEASE_HELD", `${id} is held by ${work.heldBy}`, {
            holder: work.heldBy,
          });
        }
        const children = service.children(id);
        const result = await context.events.waterfall<
          { work: WorkRecord; children: WorkRecord[]; sessionId: string },
          GateResult
        >(
          "work.start",
          { work: { ...work, heldBy: null }, children, sessionId: session.id },
          async () => ({ blocked: false }),
        );
        blockIfNeeded(result);
        const now = new Date().toISOString();
        const claimed = context.store.database
          .query(
            `UPDATE work
             SET state = 'active', held_by = ?, updated_at = ?
             WHERE id = ? AND state != 'done' AND (held_by IS NULL OR held_by = ?)`,
          )
          .run(session.id, now, id, session.id);
        if (claimed.changes === 0) {
          const current = requireWork(context, id);
          if (current.state === "done") {
            throw new CliError("INVALID_STATE", `${id} is already done`);
          }
          throw new CliError("LEASE_HELD", `${id} is held by ${current.heldBy}`, {
            holder: current.heldBy,
          });
        }
        context.log.append({
          type: "work.start",
          entityType: "work",
          entityId: id,
          sessionId: session.id,
          payload: { holder: session.id },
        });
        return { data: { work: service.get(id) }, text: `${id} started by ${session.id}` };
      }, { description: "Start work and claim its live session lease.", maxPositionals: 1 }),
    );

    context.effect(() =>
      context.cli.register("work note", (invocation): CliResult => {
        const id = requirePosition(invocation, 0, "work id");
        const text = requirePosition(invocation, 1, "note text");
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
      }, { description: "Append a note to a work item.", maxPositionals: 2 }),
    );

    context.effect(() =>
      context.cli.register(
        "work done",
        async (invocation): Promise<CliResult> => {
          const id = requirePosition(invocation, 0, "work id");
          const work = requireWork(context, id);
          const sessionId = context.sessions.current().id;
          if (work.heldBy !== sessionId) {
            if (work.heldBy) {
              throw new CliError("LEASE_HELD", `${id} is held by ${work.heldBy}`, {
                holder: work.heldBy,
              });
            }
            const command = `maestro work start ${id}`;
            const expired = latestLeaseExpiration(context, id);
            const explanation = expired
              ? `; previous lease held by ${expired.holder} expired because ${expired.reason}`
              : "";
            throw new CliError(
              "LEASE_REQUIRED",
              `${id} must be started before completion; run: ${command}${explanation}`,
              expired
                ? { command, expiredHolder: expired.holder, expiredReason: expired.reason }
                : { command },
            );
          }
          const evidence = textOption(invocation, "evidence") ?? "";
          const claims = listOption(invocation, "claim");
          const proofs = listOption(invocation, "proof");
          const result = await context.events.waterfall<
            { claims: string[]; evidence: string; proofs: string[]; work: WorkRecord },
            GateResult
          >(
            "work.done",
            { work, evidence, claims, proofs },
            async (completion) => ({ blocked: false, evidence: completion.evidence }),
          );
          blockIfNeeded(result);
          const recordedEvidence = result.evidence ?? evidence;
          const now = new Date().toISOString();
          context.store.database
            .query(
              "UPDATE work SET state = 'done', evidence = ?, held_by = NULL, updated_at = ? WHERE id = ?",
            )
            .run(recordedEvidence, now, id);
          context.log.append({
            type: "work.done",
            entityType: "work",
            entityId: id,
            sessionId,
            payload: { evidence: recordedEvidence, claims, proofs },
          });
          return { data: { work: service.get(id) }, text: `${id} done` };
        },
        {
          description: "Complete held work with policy-checked evidence.",
          flags: {
            "--evidence": {
              description: "Record opaque completion evidence.",
              value: true,
            },
          },
          maxPositionals: 1,
        },
      ),
    );

    context.effect(() =>
      context.cli.register("work show", (invocation): CliResult => {
        const work = requireWork(context, requirePosition(invocation, 0, "work id"));
        return { data: { work }, text: formatWork(work) };
      }, { description: "Show one work item and its recorded evidence.", maxPositionals: 1 }),
    );

    context.effect(() =>
      context.cli.register(
        "work list",
        (): CliResult => {
          const works = service.list();
          return {
            data: { works },
            text: works.map((work) => `${work.id} [${work.state}] ${work.title}`).join("\n"),
          };
        },
        { description: "List tracked work and current states." },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "ready",
        (): CliResult => {
          const items = service.list().map((work) => {
            const blockers = context.store.database
              .query<{ id: string; state: string }, [string]>(
                `SELECT blocker.id, blocker.state
                 FROM work_blockers edge
                 JOIN work blocker ON blocker.id = edge.blocker_id
                 WHERE edge.work_id = ?`,
              )
              .all(work.id);
            return { ...work, blockers };
          });
          const ready = context.ready.project(items);
          return {
            data: { works: ready },
            text:
              ready.length > 0
                ? ready.map((work) => `${work.id} ${work.title}`).join("\n")
                : "no ready work",
          };
        },
        { description: "List work unblocked and ready to start." },
      ),
    );
  },
};
