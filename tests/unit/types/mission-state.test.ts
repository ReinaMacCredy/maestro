import { describe, expect, it } from "bun:test";
import {
  MISSION_STATES,
  MISSION_TERMINAL_STATES,
  MISSION_TRANSITIONS,
  MissionTransitionError,
  assertMissionTransition,
  canTransitionMission,
  isMissionState,
  isTerminalMissionState,
  type MissionState,
} from "@/types/mission-state.js";

describe("MissionState union", () => {
  it("isMissionState recognizes every canonical state and rejects unknowns", () => {
    for (const state of MISSION_STATES) {
      expect(isMissionState(state)).toBe(true);
    }
    expect(isMissionState("nope")).toBe(false);
    expect(isMissionState(undefined)).toBe(false);
  });

  it("isTerminalMissionState matches the terminal list exactly", () => {
    for (const state of MISSION_STATES) {
      const expected = (MISSION_TERMINAL_STATES as readonly MissionState[]).includes(state);
      expect(isTerminalMissionState(state)).toBe(expected);
    }
  });

  it("includes paused, failed, and renamed approved", () => {
    expect((MISSION_STATES as readonly string[]).includes("paused")).toBe(true);
    expect((MISSION_STATES as readonly string[]).includes("failed")).toBe(true);
    expect((MISSION_STATES as readonly string[]).includes("approved")).toBe(true);
    expect((MISSION_STATES as readonly string[]).includes("specified")).toBe(false);
  });
});

describe("MISSION_TRANSITIONS", () => {
  it("declares no outgoing transitions from terminal states", () => {
    expect(MISSION_TRANSITIONS.completed).toEqual([]);
    expect(MISSION_TRANSITIONS.failed).toEqual([]);
    expect(MISSION_TRANSITIONS.cancelled).toEqual([]);
  });

  it("allows cancelled from every non-terminal state", () => {
    for (const state of MISSION_STATES) {
      if (isTerminalMissionState(state)) continue;
      expect(MISSION_TRANSITIONS[state]).toContain("cancelled");
    }
  });

  it("covers the canonical happy path intake -> approved -> planned -> in-progress -> completed", () => {
    expect(canTransitionMission("intake", "approved")).toBe(true);
    expect(canTransitionMission("approved", "planned")).toBe(true);
    expect(canTransitionMission("planned", "in-progress")).toBe(true);
    expect(canTransitionMission("in-progress", "completed")).toBe(true);
  });

  it("allows intake -> planned (decompose bypass)", () => {
    expect(canTransitionMission("intake", "planned")).toBe(true);
  });

  it("supports the paused round-trip (in-progress <-> paused)", () => {
    expect(canTransitionMission("in-progress", "paused")).toBe(true);
    expect(canTransitionMission("paused", "in-progress")).toBe(true);
  });

  it("permits failed from in-progress and paused but not from earlier states", () => {
    expect(canTransitionMission("in-progress", "failed")).toBe(true);
    expect(canTransitionMission("paused", "failed")).toBe(true);
    expect(canTransitionMission("intake", "failed")).toBe(false);
    expect(canTransitionMission("approved", "failed")).toBe(false);
    expect(canTransitionMission("planned", "failed")).toBe(false);
  });

  it("paused -> completed remains in the table (belt-and-suspenders for fixtures)", () => {
    expect(canTransitionMission("paused", "completed")).toBe(true);
  });

  it("forbids backward transitions", () => {
    expect(canTransitionMission("planned", "intake")).toBe(false);
    expect(canTransitionMission("in-progress", "planned")).toBe(false);
    expect(canTransitionMission("completed", "in-progress")).toBe(false);
    expect(canTransitionMission("failed", "in-progress")).toBe(false);
  });

  it("forbids skipping in-progress (planned cannot land directly in paused)", () => {
    expect(canTransitionMission("planned", "paused")).toBe(false);
  });
});

describe("assertMissionTransition", () => {
  it("returns void on valid transition", () => {
    expect(() => {
      assertMissionTransition("intake", "approved");
    }).not.toThrow();
  });

  it("throws MissionTransitionError on invalid transition", () => {
    let caught: unknown;
    try {
      assertMissionTransition("intake", "completed");
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(MissionTransitionError);
    expect((caught as MissionTransitionError).from).toBe("intake");
    expect((caught as MissionTransitionError).to).toBe("completed");
  });
});
