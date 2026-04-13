import { describe, expect, it } from "bun:test";
import {
  buildPrincipleEffectiveness,
  hasSufficientSample,
  PRINCIPLE_SMALL_SAMPLE_THRESHOLD,
} from "@/features/mission/usecases/principle-effectiveness.usecase.js";
import type {
  Principle,
  PrincipleOutcomeRecord,
} from "@/features/mission/domain/principle-types.js";

function principle(id: string): Principle {
  return {
    id,
    name: id,
    source: "custom",
    rule: "test",
    profiles: ["implementation"],
    mode: "gate",
    gateField: "assumptions",
    gateCheck: "array_min_length:1",
  };
}

function outcome(
  principleId: string,
  handoffId: string,
  outcome: "pending" | "helpful" | "unhelpful",
  recordedAt: string,
): PrincipleOutcomeRecord {
  return { principleId, handoffId, outcome, recordedAt };
}

describe("buildPrincipleEffectiveness", () => {
  it("returns empty map when no principles and no outcomes", () => {
    const result = buildPrincipleEffectiveness([], []);
    expect(result.size).toBe(0);
  });

  it("tracks every principle even with zero outcomes", () => {
    const result = buildPrincipleEffectiveness([principle("p-1")], []);
    const stats = result.get("p-1");
    expect(stats).toBeDefined();
    expect(stats!.helpful).toBe(0);
    expect(stats!.unhelpful).toBe(0);
    expect(stats!.effectiveness).toBeUndefined();
  });

  it("counts helpful and unhelpful outcomes, computes ratio", () => {
    const outcomes = [
      outcome("p-1", "h-1", "helpful", "2026-04-13T00:00:00Z"),
      outcome("p-1", "h-2", "helpful", "2026-04-13T01:00:00Z"),
      outcome("p-1", "h-3", "unhelpful", "2026-04-13T02:00:00Z"),
    ];
    const result = buildPrincipleEffectiveness([principle("p-1")], outcomes);
    const stats = result.get("p-1")!;
    expect(stats.helpful).toBe(2);
    expect(stats.unhelpful).toBe(1);
    expect(stats.effectiveness).toBeCloseTo(2 / 3);
    expect(stats.total).toBe(3);
  });

  it("collapses same-pair records to the newest state", () => {
    const outcomes = [
      outcome("p-1", "h-1", "pending", "2026-04-13T00:00:00Z"),
      outcome("p-1", "h-1", "helpful", "2026-04-13T01:00:00Z"),
      outcome("p-1", "h-1", "unhelpful", "2026-04-13T02:00:00Z"),
    ];
    const result = buildPrincipleEffectiveness([principle("p-1")], outcomes);
    const stats = result.get("p-1")!;
    expect(stats.helpful).toBe(0);
    expect(stats.unhelpful).toBe(1);
    expect(stats.pending).toBe(0);
  });

  it("tracks pending outcomes separately from decided counts", () => {
    const outcomes = [
      outcome("p-1", "h-1", "pending", "2026-04-13T00:00:00Z"),
      outcome("p-1", "h-2", "helpful", "2026-04-13T01:00:00Z"),
    ];
    const result = buildPrincipleEffectiveness([principle("p-1")], outcomes);
    const stats = result.get("p-1")!;
    expect(stats.helpful).toBe(1);
    expect(stats.pending).toBe(1);
    expect(stats.total).toBe(2);
    // Effectiveness ignores pending
    expect(stats.effectiveness).toBe(1);
  });

  it("still tallies outcomes for principles removed from the store", () => {
    const outcomes = [
      outcome("p-removed", "h-1", "helpful", "2026-04-13T00:00:00Z"),
      outcome("p-removed", "h-2", "unhelpful", "2026-04-13T01:00:00Z"),
    ];
    const result = buildPrincipleEffectiveness([], outcomes);
    expect(result.get("p-removed")?.helpful).toBe(1);
    expect(result.get("p-removed")?.unhelpful).toBe(1);
  });
});

describe("hasSufficientSample", () => {
  it(`returns true at or above the ${PRINCIPLE_SMALL_SAMPLE_THRESHOLD} threshold`, () => {
    expect(hasSufficientSample({
      principleId: "p",
      helpful: 2,
      unhelpful: 1,
      pending: 0,
      total: 3,
    })).toBe(true);
  });

  it("returns false below threshold", () => {
    expect(hasSufficientSample({
      principleId: "p",
      helpful: 1,
      unhelpful: 1,
      pending: 5,
      total: 7,
    })).toBe(false);
  });
});
