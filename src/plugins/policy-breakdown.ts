import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { WorkRecord } from "./work.ts";

const writeLikeKinds = new Set(["feature", "task", "bug", "chore", "implement"]);

export const policyBreakdownPlugin: BuiltInPlugin = {
  name: "policy-breakdown",
  inject: ["work"],
  apply(context) {
    context.effect(() =>
      context.events.on<{ children: WorkRecord[]; work: WorkRecord }>(
        "work.start",
        async (input, next) => {
          if (
            writeLikeKinds.has(input.work.kind) &&
            !input.work.parentId &&
            input.children.length === 0 &&
            !input.work.atomicReason
          ) {
            return {
              blocked: true,
              origin: "policy-breakdown",
              reason: "parentless write-like work requires a child breakdown or atomic reason",
            };
          }
          return next();
        },
      ),
    );
  },
};
