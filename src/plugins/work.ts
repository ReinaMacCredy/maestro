import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";

export interface WorkRecord {
  id: string;
  title: string;
  kind: string;
  state: "open" | "active" | "done" | "cancelled";
  parentId: string | null;
  acceptance: string | null;
  atomicReason: string | null;
  evidence: string | null;
  heldBy: string | null;
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
  held_by: string | null;
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
}

export interface WorkGateInput {
  children: WorkRecord[];
  sessionId: string;
  work: WorkRecord;
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
    state: row.cancelled_at ? "cancelled" : row.state,
    parentId: row.parent_id,
    acceptance: row.acceptance,
    atomicReason: row.atomic_reason,
    evidence: row.evidence,
    heldBy: row.held_by,
    cancelReason: row.cancel_reason,
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
    work.acceptance ? `acceptance: ${work.acceptance}` : null,
    work.atomicReason ? `atomic reason: ${work.atomicReason}` : null,
    work.cancelReason ? `cancel reason: ${work.cancelReason}` : null,
    work.evidence !== null ? `evidence: ${work.evidence}` : null,
  ];
  return fields.filter((field): field is string => field !== null).join("\n");
}

function blockIfNeeded(result: GateResult): void {
  if (!result.blocked) return;
  const details = gateDetails(result);
  throw new CliError("GATE_BLOCKED", details.reason, { origin: details.origin });
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
    const hasWorkColumn = (name: string) =>
      context.store.database
        .query<{ name: string }, []>("PRAGMA table_info(work)")
        .all()
        .some((column) => column.name === name);
    for (const [name, migration] of [
      ["cancelled_at", "ALTER TABLE work ADD COLUMN cancelled_at TEXT"],
      ["cancel_reason", "ALTER TABLE work ADD COLUMN cancel_reason TEXT"],
    ] as const) {
      if (hasWorkColumn(name)) continue;
      try {
        context.store.migrate(migration);
      } catch (error) {
        // Concurrent startup can race the same ALTER; losing is fine.
        if (!hasWorkColumn(name)) throw error;
      }
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
    };
    context.effect(() => context.provide("work", service));

    context.effect(() =>
      context.cli.register(
        "work add",
        (invocation): CliResult => {
          const title = requirePosition(invocation, 0, "work title");
          const parentId = textOption(invocation, "parent") ?? null;
          const blockers = listOption(invocation, "blocked-by");
          for (const blocker of blockers) requireWork(context, blocker);
          let id = "";
          const now = new Date().toISOString();
          const kind = textOption(invocation, "kind") ?? "task";
          const acceptance = textOption(invocation, "acceptance") ?? null;
          const atomicReason = textOption(invocation, "atomic-reason") ?? null;
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
          },
          positionals: [{ name: "title", required: true }],
          rootDescription: "Manage tracked work, leases, dependencies, and evidence.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register("work start", async (invocation): Promise<CliResult> => {
        const id = requirePosition(invocation, 0, "work id");
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
        context.sessions.record("work.start");
        context.log.append({
          type: "work.start",
          entityType: "work",
          entityId: id,
          sessionId: session.id,
          payload: { holder: session.id },
        });
        return { data: { work: service.get(id) }, text: `${id} started by ${session.id}` };
      }, {
        description: "Start work and claim its live session lease.",
        positionals: [{ name: "id", required: true }],
      }),
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
      }, {
        description: "Append a note to a work item.",
        positionals: [
          { name: "id", required: true },
          { name: "text", required: true },
        ],
      }),
    );

    context.effect(() =>
      context.cli.register(
        "work done",
        async (invocation): Promise<CliResult> => {
          const id = requirePosition(invocation, 0, "work id");
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
            { work, children: service.children(id), evidence, claims, proofs, sessionId },
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
          context.sessions.record("work.done");
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
          positionals: [{ name: "id", required: true }],
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "work cancel",
        (invocation): CliResult => {
          const id = requirePosition(invocation, 0, "work id");
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
          const reason = textOption(invocation, "reason");
          if (!reason) {
            throw new CliError("MISSING_ARGUMENT", "work cancel requires --reason <text>");
          }
          const now = new Date().toISOString();
          context.store.database
            .query(
              "UPDATE work SET cancelled_at = ?, cancel_reason = ?, held_by = NULL, updated_at = ? WHERE id = ?",
            )
            .run(now, reason, now, id);
          context.log.append({
            type: "work.cancel",
            entityType: "work",
            entityId: id,
            sessionId: context.sessions.current().id,
            payload: { reason },
          });
          return { data: { work: service.get(id) }, text: `${id} cancelled: ${reason}` };
        },
        {
          description: "Cancel open or currently held work permanently with a recorded reason.",
          flags: {
            "--reason": { description: "Record why this work is cancelled.", value: true },
          },
          positionals: [{ name: "id", required: true }],
        },
      ),
    );

    context.effect(() =>
      context.cli.register("work show", (invocation): CliResult => {
        const work = requireWork(context, requirePosition(invocation, 0, "work id"));
        const children = service.children(work.id);
        const notes = context.store.database
          .query<WorkNoteRow, [string]>(
            "SELECT text, created_at FROM work_notes WHERE work_id = ? ORDER BY id",
          )
          .all(work.id)
          .map((note) => ({ text: note.text, createdAt: note.created_at }));
        const childLines = children.map(
          (child) => `child: ${child.id} [${child.state}] ${child.title}`,
        );
        return {
          data: { work, children, notes },
          text: [formatWork(work), ...childLines, ...notes.map((note) => `note: ${note.text}`)].join("\n"),
        };
      }, {
        description: "Show one work item with its evidence, children, and notes.",
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
        { description: "List tracked work and current states." },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "ready",
        async (): Promise<CliResult> => {
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
                blockers: work.blockers.map((blocker) => ({
                  id: blocker.id,
                  state: blocker.state,
                })),
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
          const lines = [
            ...works.map((work) => `${work.id} ${work.title}`),
            ...gated.map(
              (work) => `${work.id} ${work.title} [gated by ${work.origin}: ${work.reason}]`,
            ),
          ];
          const held = allWorks.filter(
            (work) => work.state === "active" && work.heldBy === sessionId,
          );
          const allTerminal = allWorks.length > 0 && allWorks.every(
            (work) => work.state === "done" || work.state === "cancelled",
          );
          return {
            data: { works, gated },
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
        { description: "List ready work and gated items with their blockers." },
      ),
    );
  },
};
