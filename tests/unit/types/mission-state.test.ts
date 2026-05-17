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
});

describe("MISSION_TRANSITIONS", () => {
  it("declares no outgoing transitions from terminal states", () => {
    expect(MISSION_TRANSITIONS.completed).toEqual([]);
    expect(MISSION_TRANSITIONS.cancelled).toEqual([]);
  });

  it("allows cancelled from every non-terminal state", () => {
    for (const state of MISSION_STATES) {
      if (isTerminalMissionState(state)) continue;
      expect(MISSION_TRANSITIONS[state]).toContain("cancelled");
    }
  });

  it("covers the canonical happy path intake -> specified -> planned -> in-progress -> completed", () => {
    expect(canTransitionMission("intake", "specified")).toBe(true);
    expect(canTransitionMission("specified", "planned")).toBe(true);
    expect(canTransitionMission("planned", "in-progress")).toBe(true);
    expect(canTransitionMission("in-progress", "completed")).toBe(true);
  });

  it("forbids backward transitions", () => {
    expect(canTransitionMission("planned", "intake")).toBe(false);
    expect(canTransitionMission("in-progress", "planned")).toBe(false);
    expect(canTransitionMission("completed", "in-progress")).toBe(false);
  });
});

describe("assertMissionTransition", () => {
  it("returns void on valid transition", () => {
    expect(() => {
      assertMissionTransition("intake", "specified");
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
