import {
  CliError,
  requiredPosition,
  stringOption,
  type CliResult,
} from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";
import { registerSessionCommand } from "../session-required.ts";
import {
  blockerDetails,
  blockIfNeeded,
  requireWork,
  unresolvedBlockers,
  type GateResult,
  type WorkGateInput,
  type WorkService,
} from "./records.ts";

export function registerWorkStart(context: PluginContext, service: WorkService): void {
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
}

export function registerWorkRelease(context: PluginContext, service: WorkService): void {
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
}

export function registerWorkReclaim(context: PluginContext, service: WorkService): void {
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
}
