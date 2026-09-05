import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { DispatchService } from "./dispatch.ts";
import type { WorkAddGateInput, WorkRecord, WorkService } from "./work.ts";

const defaultLimit = 3;

function limitFrom(config: unknown): number {
  const limit = (config as { limit?: unknown } | undefined)?.limit;
  return typeof limit === "number" && Number.isInteger(limit) && limit > 0 ? limit : defaultLimit;
}

// A card is attended when a live session holds it or a lane has accepted a
// dispatch on it. An open dispatch nobody accepted is not a worker (d703).
// A graph run (kind graph, Hub d79) is machine work; one whose driver died
// would otherwise hold a slot for ever (graph-engine A4, advisor F9).
function unattended(
  works: WorkRecord[],
  dispatch: DispatchService,
  isAlive: (id: string) => boolean,
): WorkRecord[] {
  const accepted = new Set(
    dispatch
      .list()
      .filter((record) => record.state === "open" && (record.heldBy || record.claimedBy))
      .map((record) => record.workId),
  );
  return works.filter(
    (work) =>
      work.kind !== "graph" &&
      (work.state === "open" || work.state === "active") &&
      !(work.heldBy && isAlive(work.heldBy)) &&
      !accepted.has(work.id),
  );
}

export const policyCardBudgetPlugin: BuiltInPlugin = {
  name: "policy-card-budget",
  defaultDisabled: true,
  inject: ["work", "dispatch"],
  requires:
    "gates work add: refuses a new card while the store already holds <limit> (default 3) open cards with no live holder and no accepted dispatch; counts the store, not the session, so release, --parent and a rotated session id do not reset it; command-path only, raw SQL is not covered",
  apply(context, config) {
    const limit = limitFrom(config);
    const work = context.work as WorkService;
    const dispatch = context.dispatch as DispatchService;
    context.effect(() =>
      context.events.on<WorkAddGateInput>("work.add", async (_input, next) => {
        const idle = unattended(work.list(), dispatch, (id) => context.sessions.isAlive(id));
        if (idle.length < limit) return next();
        const ids = idle.map((record) => record.id);
        return {
          blocked: true,
          blockers: idle.map((record) => ({ id: record.id, state: record.state })),
          origin: "policy-card-budget",
          reason:
            `the store already holds ${idle.length} cards with nobody behind them: ${ids.join(", ")}; ` +
            `finish, dispatch, or cancel one first: maestro work done <id> | ` +
            `maestro dispatch open <id> ... | maestro work cancel <id> --reason "<why>"`,
        };
      })
    );
  },
};
