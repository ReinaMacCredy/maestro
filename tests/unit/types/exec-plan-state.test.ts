import { describe, expect, it } from "bun:test";
import {
  EXEC_PLAN_STATES,
  EXEC_PLAN_TERMINAL_STATES,
  EXEC_PLAN_TRANSITIONS,
  ExecPlanTransitionError,
  assertExecPlanTransition,
  canTransitionExecPlan,
  isExecPlanState,
  isTerminalExecPlanState,
  type ExecPlanState,
} from "@/types/exec-plan-state.js";

describe("ExecPlanState union", () => {
  it("isExecPlanState recognizes every canonical state and rejects unknowns", () => {
    for (const state of EXEC_PLAN_STATES) {
      expect(isExecPlanState(state)).toBe(true);
    }
    expect(isExecPlanState("nope")).toBe(false);
    expect(isExecPlanState(undefined)).toBe(false);
  });

  it("isTerminalExecPlanState matches the terminal list exactly", () => {
    for (const state of EXEC_PLAN_STATES) {
      const expected = (EXEC_PLAN_TERMINAL_STATES as readonly ExecPlanState[]).includes(state);
      expect(isTerminalExecPlanState(state)).toBe(expected);
    }
  });
});

describe("EXEC_PLAN_TRANSITIONS", () => {
  it("declares no outgoing transitions from terminal states", () => {
    expect(EXEC_PLAN_TRANSITIONS.completed).toEqual([]);
    expect(EXEC_PLAN_TRANSITIONS.cancelled).toEqual([]);
  });

  it("allows cancelled from every non-terminal state", () => {
    for (const state of EXEC_PLAN_STATES) {
      if (isTerminalExecPlanState(state)) continue;
      expect(EXEC_PLAN_TRANSITIONS[state]).toContain("cancelled");
    }
  });

  it("covers the canonical happy path intake -> specified -> planned -> in-progress -> completed", () => {
    expect(canTransitionExecPlan("intake", "specified")).toBe(true);
    expect(canTransitionExecPlan("specified", "planned")).toBe(true);
    expect(canTransitionExecPlan("planned", "in-progress")).toBe(true);
    expect(canTransitionExecPlan("in-progress", "completed")).toBe(true);
  });

  it("forbids backward transitions", () => {
    expect(canTransitionExecPlan("planned", "intake")).toBe(false);
    expect(canTransitionExecPlan("in-progress", "planned")).toBe(false);
    expect(canTransitionExecPlan("completed", "in-progress")).toBe(false);
  });
});

describe("assertExecPlanTransition", () => {
  it("returns void on valid transition", () => {
    expect(() => {
      assertExecPlanTransition("intake", "specified");
    }).not.toThrow();
  });

  it("throws ExecPlanTransitionError on invalid transition", () => {
    let caught: unknown;
    try {
      assertExecPlanTransition("intake", "completed");
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(ExecPlanTransitionError);
    expect((caught as ExecPlanTransitionError).from).toBe("intake");
    expect((caught as ExecPlanTransitionError).to).toBe("completed");
  });
});
