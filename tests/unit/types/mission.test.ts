import { describe, expect, it } from "bun:test";
import { MISSION_ID_PATTERN, generateMissionId, isMissionId } from "@/types/mission.js";

describe("Mission id helpers", () => {
  it("generateMissionId produces ids matching the pln-<ts>-<rand> shape", () => {
    const id = generateMissionId();
    expect(id.startsWith("pln-")).toBe(true);
    expect(MISSION_ID_PATTERN.test(id)).toBe(true);
  });

  it("isMissionId accepts generated ids", () => {
    for (let i = 0; i < 8; i++) {
      expect(isMissionId(generateMissionId())).toBe(true);
    }
  });

  it("isMissionId rejects task-shaped or arbitrary strings", () => {
    expect(isMissionId("tsk-abc-def")).toBe(false);
    expect(isMissionId("pln-")).toBe(false);
    expect(isMissionId("pln-abc")).toBe(false);
    expect(isMissionId("")).toBe(false);
    expect(isMissionId(undefined)).toBe(false);
    expect(isMissionId(42)).toBe(false);
  });

  it("generated ids are unique under back-to-back calls", () => {
    const ids = new Set<string>();
    for (let i = 0; i < 16; i++) ids.add(generateMissionId());
    expect(ids.size).toBeGreaterThan(1);
  });
});
