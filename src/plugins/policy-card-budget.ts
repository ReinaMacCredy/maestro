import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { WorkAddGateInput, WorkRecord, WorkService } from "./work.ts";

const defaultLimit = 3;

function limitFrom(config: unknown): number {
  const limit = (config as { limit?: unknown } | undefined)?.limit;
  return typeof limit === "number" && Number.isInteger(limit) && limit > 0 ? limit : defaultLimit;
}

// A card is attended when a live session holds it (d703).
function unattended(
  works: WorkRecord[],
  isAlive: (id: string) => boolean,
): WorkRecord[] {
  return works.filter(
    (work) =>
      (work.state === "open" || work.state === "active") &&
      !(work.heldBy && isAlive(work.heldBy)),
  );
}

export const policyCardBudgetPlugin: BuiltInPlugin = {
  name: "policy-card-budget",
  defaultDisabled: true,
  inject: ["work"],
  requires:
    "gates work add: refuses a new card while the store already holds <limit> (default 3) open cards with no live holder; counts the store, not the session, so release, --parent and a rotated session id do not reset it; command-path only, raw SQL is not covered",
  apply(context, config) {
    const limit = limitFrom(config);
    const work = context.work as WorkService;
    context.effect(() =>
      context.events.on<WorkAddGateInput>("work.add", async (_input, next) => {
        const idle = unattended(work.list(), (id) => context.sessions.isAlive(id));
        if (idle.length < limit) return next();
        const ids = idle.map((record) => record.id);
        return {
          blocked: true,
          blockers: idle.map((record) => ({ id: record.id, state: record.state })),
          origin: "policy-card-budget",
          reason:
            `the store already holds ${idle.length} cards with nobody behind them: ${ids.join(", ")}; ` +
            `finish or cancel one first: maestro work done <id> | ` +
            `maestro work cancel <id> --reason "<why>"`,
        };
      })
    );
  },
};
