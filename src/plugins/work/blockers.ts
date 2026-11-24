import {
  CliError,
  requiredPosition,
  stringOptions,
  type CliInvocation,
  type CliResult,
} from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";
import { registerSessionCommand } from "../session-required.ts";
import { blockersFor, requireWork, type WorkRecord, type WorkService } from "./records.ts";

export function registerWorkBlockers(context: PluginContext, service: WorkService): void {
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
}
