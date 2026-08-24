import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { WorkGateInput, WorkRecord } from "./work.ts";

const writeLikeKinds = new Set(["feature", "task", "bug", "chore", "implement"]);

// Cancelled children are not a breakdown; without this filter two commands
// (add child, cancel child) would defeat the gate.
function needsBreakdown(work: WorkRecord, children: WorkRecord[]): boolean {
  return writeLikeKinds.has(work.kind) &&
    !work.parentId &&
    children.filter((child) => child.state !== "cancelled").length === 0 &&
    !work.atomicReason;
}

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
  origin: string;
  reason: string;
}

function breakdownGate(work: WorkRecord): Gate {
  return {
    blocked: true,
    origin: "policy-breakdown",
    reason:
      `parentless write-like work requires a child breakdown; run: maestro work add ` +
      `"<child>" --parent ${work.id} --kind task; for new atomic work use ` +
      `--atomic-reason "<reason>"`,
  };
}

function openChildrenGate(work: WorkRecord, open: WorkRecord[], sessionId: string): Gate {
  const ids = open.map((child) => child.id);
  const next = open.find(
    (child) => child.state === "active" && child.heldBy === sessionId,
  ) ?? open.find((child) => child.state === "active") ?? open[0] as WorkRecord;
  const command = next.state === "active"
    ? next.heldBy === sessionId
      ? `maestro work done ${next.id}`
      : "maestro status"
    : `maestro work start ${next.id}`;
  return {
    blocked: true,
    origin: "policy-breakdown",
    reason:
      `${work.id} has open children: ${ids.join(", ")}; finish them first: ` +
      command,
  };
}

function gateFor(work: WorkRecord, children: WorkRecord[], sessionId: string): Gate | null {
  if (needsBreakdown(work, children)) return breakdownGate(work);
  const open = openChildren(children);
  return open.length > 0 ? openChildrenGate(work, open, sessionId) : null;
}

export const policyBreakdownPlugin: BuiltInPlugin = {
  name: "policy-breakdown",
  inject: ["work"],
  requires:
    "gates work start/ready/done: parentless write-like work needs a child breakdown or --atomic-reason; open write-like children block their parent",
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
