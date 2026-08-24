import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { WorkGateInput, WorkRecord } from "./work.ts";

const writeLikeKinds = new Set(["feature", "task", "bug", "chore", "implement"]);

function needsBreakdown(work: WorkRecord, children: WorkRecord[]): boolean {
  return writeLikeKinds.has(work.kind) &&
    !work.parentId &&
    children.length === 0 &&
    !work.atomicReason;
}

function breakdownGate(work: WorkRecord): {
  blocked: true;
  origin: string;
  reason: string;
} {
  return {
    blocked: true,
    origin: "policy-breakdown",
    reason:
      `parentless write-like work requires a child breakdown; run: maestro work add ` +
      `"<child>" --parent ${work.id} --kind task; for new atomic work use ` +
      `--atomic-reason "<reason>"`,
  };
}

export const policyBreakdownPlugin: BuiltInPlugin = {
  name: "policy-breakdown",
  inject: ["work"],
  apply(context) {
    context.effect(() =>
      context.events.on<WorkGateInput>(
        "work.start",
        async (input, next) => {
          return needsBreakdown(input.work, input.children)
            ? breakdownGate(input.work)
            : next();
        },
      ),
    );
    context.effect(() =>
      context.events.on<WorkGateInput>(
        "work.ready",
        async (input, next) => {
          return needsBreakdown(input.work, input.children)
            ? breakdownGate(input.work)
            : next();
        },
      ),
    );
  },
};
