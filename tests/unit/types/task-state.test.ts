import { describe, expect, it } from "bun:test";
import {
  TASK_STATES,
  TASK_TERMINAL_STATES,
  TASK_TRANSITIONS,
  TaskTransitionError,
  assertTaskTransition,
  canTransitionTask,
  isTaskState,
  isTerminalTaskState,
  type TaskState,
} from "@/v2/types/task-state.js";

describe("TaskState union", () => {
  it("isTaskState recognizes every canonical state and rejects unknowns", () => {
    for (const state of TASK_STATES) {
      expect(isTaskState(state)).toBe(true);
    }
    expect(isTaskState("nope")).toBe(false);
    expect(isTaskState(undefined)).toBe(false);
    expect(isTaskState(42)).toBe(false);
  });

  it("isTerminalTaskState matches the terminal list exactly", () => {
    for (const state of TASK_STATES) {
      const expected = (TASK_TERMINAL_STATES as readonly TaskState[]).includes(state);
      expect(isTerminalTaskState(state)).toBe(expected);
    }
  });
});

describe("TASK_TRANSITIONS", () => {
  it("declares no outgoing transitions from terminal states", () => {
    expect(TASK_TRANSITIONS.shipped).toEqual([]);
    expect(TASK_TRANSITIONS.abandoned).toEqual([]);
  });

  it("allows abandon from every non-terminal state", () => {
    for (const state of TASK_STATES) {
      if (isTerminalTaskState(state)) continue;
      expect(TASK_TRANSITIONS[state]).toContain("abandoned");
    }
  });

  it("covers the canonical happy path draft -> claimed -> doing -> verifying -> ready -> shipped", () => {
    expect(canTransitionTask("draft", "claimed")).toBe(true);
    expect(canTransitionTask("claimed", "doing")).toBe(true);
    expect(canTransitionTask("doing", "verifying")).toBe(true);
    expect(canTransitionTask("verifying", "ready")).toBe(true);
    expect(canTransitionTask("ready", "shipped")).toBe(true);
  });

  it("supports the Ralph Wiggum loop verifying <-> doing", () => {
    expect(canTransitionTask("verifying", "doing")).toBe(true);
    expect(canTransitionTask("doing", "verifying")).toBe(true);
  });
});

describe("assertTaskTransition", () => {
  it("returns void on valid transition", () => {
    expect(() => {
      assertTaskTransition("draft", "claimed");
    }).not.toThrow();
  });

  it("throws TaskTransitionError on invalid transition with allowed list in message", () => {
    let caught: unknown;
    try {
      assertTaskTransition("draft", "shipped");
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(TaskTransitionError);
    expect((caught as TaskTransitionError).from).toBe("draft");
    expect((caught as TaskTransitionError).to).toBe("shipped");
    expect((caught as TaskTransitionError).message).toContain("allowed from draft");
  });

  it("throws with 'terminal' hint when source state has no transitions", () => {
    let caught: unknown;
    try {
      assertTaskTransition("shipped", "doing");
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(TaskTransitionError);
    expect((caught as TaskTransitionError).message).toContain("terminal");
  });
});
