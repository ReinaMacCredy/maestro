import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { DispatchService } from "./dispatch.ts";
import type { WorkGateInput } from "./work.ts";

interface Gate {
  blocked: true;
  command: string;
  origin: string;
  reason: string;
}

function doneGate(dispatch: DispatchService, workId: string): Gate | null {
  const pending = dispatch.list(workId).filter((record) => record.state === "open");
  if (pending.length === 0) return null;
  const first = pending[0] as (typeof pending)[number];
  const command = `maestro dispatch cancel ${first.id} --reason "<reason>"`;
  return {
    blocked: true,
    command,
    origin: "policy-dispatch",
    reason:
      `${workId} has dispatches without handbacks: ${pending.map((record) => record.id).join(", ")}; ` +
      `file their handbacks or run: ${command}`,
  };
}

function laneGate(dispatch: DispatchService, workId: string, sessionId: string): Gate | null {
  const held = dispatch
    .list(workId)
    .find((record) => record.state === "open" && record.heldBy === sessionId && record.lane !== "delivery");
  if (!held) return null;
  const command = `maestro handback file ${held.id} --status DONE ...`;
  return {
    blocked: true,
    command,
    origin: "policy-dispatch",
    reason:
      `${sessionId} holds ${held.id}, a ${held.lane} lane, which is no-write on ${workId}; ` +
      `return by handback instead: ${command}`,
  };
}

function startGate(dispatch: DispatchService, workId: string): Gate | null {
  const council = dispatch.council(workId);
  if (!council.sealed) return null;
  const command = `maestro dispatch list ${workId}`;
  return {
    blocked: true,
    command,
    origin: "policy-dispatch",
    reason:
      `${workId} has a sealed council (${council.returned}/${council.total} returned); ` +
      `resolve or cancel every lane before implementation; inspect: ${command}`,
  };
}

export const policyDispatchPlugin: BuiltInPlugin = {
  name: "policy-dispatch",
  inject: ["work", "dispatch"],
  requires:
    "gates work done and work cancel while a dispatch lacks a handback, and work start while a sealed council is open or the session holds a no-write lane",
  apply(context) {
    const dispatch = context.dispatch as DispatchService;
    context.effect(() =>
      context.events.on<WorkGateInput>("work.done", async (input, next) =>
        doneGate(dispatch, input.work.id) ?? next()
      ),
    );
    context.effect(() =>
      context.events.on<WorkGateInput>("work.cancel", async (input, next) =>
        doneGate(dispatch, input.work.id) ?? next()
      ),
    );
    context.effect(() =>
      context.events.on<WorkGateInput>("work.start", async (input, next) =>
        startGate(dispatch, input.work.id) ?? laneGate(dispatch, input.work.id, input.sessionId) ?? next()
      ),
    );
  },
};
