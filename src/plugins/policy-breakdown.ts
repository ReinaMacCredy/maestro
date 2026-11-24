import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { WorkGateInput, WorkRecord } from "./work.ts";

const writeLikeKinds = new Set(["feature", "task", "bug", "chore", "implement"]);

// Only write-like children are execution units; idea-like children are scope
// notes and never hold their parent open.
function openChildren(children: WorkRecord[]): WorkRecord[] {
  return children.filter(
    (child) =>
      writeLikeKinds.has(child.kind) &&
      (child.state === "open" || child.state === "active"),
  );
}

interface Gate {
  blocked: true;
  blockers?: Array<{ id: string; state: string }>;
  command?: string;
  origin: string;
  reason: string;
}

function openChildrenGate(work: WorkRecord, open: WorkRecord[], sessionId: string): Gate {
  const ids = open.map((child) => child.id);
  const next = open.find(
    (child) => child.state === "active" && child.heldBy === sessionId,
  ) ?? open.find((child) => child.state === "active") ?? open[0] as WorkRecord;
  const command = next.state === "active"
    ? next.heldBy === sessionId
      ? `maestro work done ${next.id}`
      : `maestro work list --parent ${work.id}`
    : `maestro work start ${next.id}`;
  return {
    blocked: true,
    blockers: open.map((child) => ({ id: child.id, state: child.state })),
    command,
    origin: "policy-breakdown",
    reason:
      `${work.id} has open children: ${ids.join(", ")}; finish them first: ` +
      command,
  };
}

function gateFor(work: WorkRecord, children: WorkRecord[], sessionId: string): Gate | null {
  const open = openChildren(children);
  return open.length > 0 ? openChildrenGate(work, open, sessionId) : null;
}

export const policyBreakdownPlugin: BuiltInPlugin = {
  name: "policy-breakdown",
  inject: ["work"],
  requires:
    "gates work start/ready/done: open write-like children block their parent",
  apply(context) {
    for (const event of ["work.start", "work.ready", "work.done"] as const) {
      context.effect(() =>
        context.events.on<WorkGateInput>(
          event,
          async (input, next) => gateFor(input.work, input.children, input.sessionId) ?? next(),
        )
      );
    }
  },
};
