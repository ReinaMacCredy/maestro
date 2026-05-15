import { describe, it, expect } from "bun:test";
import { scoreSpec } from "@/shared/domain/legacy-spec/index.js";
import type { Spec } from "@/shared/domain/legacy-spec/index.js";

function makeSpec(overrides: Partial<Pick<Spec, "acceptance_criteria" | "non_goals">>): Spec {
  return {
    schema_version: 2,
    mission_id: "2026-05-05-001",
    acceptance_criteria: [],
    non_goals: [],
    runtime_signals: [],
    created_at: "2026-05-05T00:00:00.000Z",
    updated_at: "2026-05-05T00:00:00.000Z",
    ...overrides,
  };
}

describe("scoreSpec", () => {
  it("empty spec → score 0.0, both slots missing", () => {
    const result = scoreSpec(makeSpec({}));
    expect(result.score).toBe(0.0);
    expect(result.populatedSlots).toEqual([]);
    expect(result.missingSlots).toEqual(["acceptance_criteria", "non_goals"]);
  });

  it("acceptance_criteria only → score 0.5, non_goals missing", () => {
    const result = scoreSpec(
      makeSpec({ acceptance_criteria: [{ id: "cr-1", text: "Tests pass" }] }),
    );
    expect(result.score).toBe(0.5);
    expect(result.populatedSlots).toEqual(["acceptance_criteria"]);
    expect(result.missingSlots).toEqual(["non_goals"]);
  });

  it("both populated → score 1.0, no missing slots", () => {
    const result = scoreSpec(
      makeSpec({
        acceptance_criteria: [{ id: "cr-1", text: "Tests pass" }],
        non_goals: [{ text: "No new dependencies" }],
      }),
    );
    expect(result.score).toBe(1.0);
    expect(result.populatedSlots).toEqual(["acceptance_criteria", "non_goals"]);
    expect(result.missingSlots).toEqual([]);
  });
});
