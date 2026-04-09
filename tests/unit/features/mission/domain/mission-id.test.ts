import { describe, expect, it } from "bun:test";
import { generateMissionId } from "@/domain/mission-id.js";

describe("generateMissionId", () => {
  const fixedDate = new Date("2026-03-28T12:00:00Z");

  it("generates first ID of the day as 001", () => {
    const id = generateMissionId([], fixedDate);
    expect(id).toBe("2026-03-28-001");
  });

  it("increments sequence for existing IDs on the same day", () => {
    const existing = ["2026-03-28-001", "2026-03-28-002"];
    const id = generateMissionId(existing, fixedDate);
    expect(id).toBe("2026-03-28-003");
  });

  it("ignores IDs from other days", () => {
    const existing = ["2026-03-27-005", "2026-03-26-001"];
    const id = generateMissionId(existing, fixedDate);
    expect(id).toBe("2026-03-28-001");
  });

  it("handles gaps in sequence", () => {
    const existing = ["2026-03-28-001", "2026-03-28-005"];
    const id = generateMissionId(existing, fixedDate);
    expect(id).toBe("2026-03-28-006");
  });

  it("pads single-digit sequences with zeros", () => {
    const id = generateMissionId([], fixedDate);
    expect(id).toMatch(/^\d{4}-\d{2}-\d{2}-\d{3}$/);
  });

  it("handles date rollover", () => {
    const midnight = new Date("2026-04-01T00:00:00Z");
    const existing = ["2026-03-31-010"];
    const id = generateMissionId(existing, midnight);
    expect(id).toBe("2026-04-01-001");
  });

  it("uses current date when no date provided", () => {
    const id = generateMissionId([]);
    const today = new Date().toISOString().slice(0, 10);
    expect(id.startsWith(today)).toBe(true);
  });
});
