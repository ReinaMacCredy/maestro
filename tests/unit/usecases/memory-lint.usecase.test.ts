import { describe, it, expect } from "bun:test";
import { mockCorrectionStore, mockLearningStore, mockRatchetStore } from "../../helpers/mocks.js";
import { lintMemory } from "@/usecases/memory-lint.usecase.js";
import type { Correction } from "@/domain/memory-types.js";

function makeCorrection(overrides: Partial<Correction> = {}): Correction {
  return {
    id: "c1",
    rule: "test rule",
    source: "test source",
    trigger: { keywords: ["test"], fileGlobs: [] },
    severity: "soft",
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    ...overrides,
  };
}

describe("lintMemory", () => {
  it("reports healthy when no issues", async () => {
    const result = await lintMemory(
      mockCorrectionStore([makeCorrection()]),
      mockLearningStore(),
      mockRatchetStore(),
    );
    expect(result.healthy).toBe(true);
    expect(result.warnings.length).toBe(0);
  });

  it("warns about corrections with no triggers", async () => {
    const result = await lintMemory(
      mockCorrectionStore([makeCorrection({ trigger: { keywords: [], fileGlobs: [] } })]),
      mockLearningStore(),
      mockRatchetStore(),
    );
    expect(result.warnings.some((w) => w.message.includes("no trigger"))).toBe(true);
  });

  it("warns about duplicate keywords", async () => {
    const result = await lintMemory(
      mockCorrectionStore([
        makeCorrection({ id: "c1", trigger: { keywords: ["npm"], fileGlobs: [] } }),
        makeCorrection({ id: "c2", trigger: { keywords: ["npm"], fileGlobs: [] } }),
      ]),
      mockLearningStore(),
      mockRatchetStore(),
    );
    expect(result.warnings.some((w) => w.message.includes("npm"))).toBe(true);
  });

  it("warns about uncompiled learnings", async () => {
    const store = mockLearningStore([
      { sessionDate: "2026-04-05", content: "something" },
    ]);

    const result = await lintMemory(
      mockCorrectionStore(),
      store,
      mockRatchetStore(),
    );
    expect(result.warnings.some((w) => w.message.includes("never been compiled"))).toBe(true);
  });

  it("warns about ratchet assertions referencing deleted corrections", async () => {
    const result = await lintMemory(
      mockCorrectionStore(),
      mockLearningStore(),
      mockRatchetStore({
        assertions: [{ id: "r1", correctionId: "deleted-id", rule: "test", check: "pattern", createdAt: "2026-04-05" }],
      }),
    );
    expect(result.warnings.some((w) => w.message.includes("deleted correction"))).toBe(true);
  });
});
