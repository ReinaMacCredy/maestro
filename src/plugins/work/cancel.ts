import {
  CliError,
  requiredPosition,
  stringOption,
  type CliResult,
} from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";
import { registerSessionCommand } from "../session-required.ts";
import { blockIfNeeded, requireWork, type GateResult, type WorkGateInput, type WorkService } from "./records.ts";

export function registerWorkCancel(context: PluginContext, service: WorkService): void {
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
}
