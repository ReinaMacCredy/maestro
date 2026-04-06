import { describe, it, expect } from "bun:test";
import { mockCorrectionStore, mockLearningStore } from "../../helpers/mocks.js";
import { recallMemory } from "../../../src/usecases/memory-recall.usecase.js";
import type { Correction } from "../../../src/domain/memory-types.js";

function makeCorrection(overrides: Partial<Correction> = {}): Correction {
  return {
    id: "corr-1",
    rule: "use bun not npm",
    source: "used npm install",
    trigger: { keywords: ["package", "npm", "install"], fileGlobs: ["*.sh"] },
    severity: "soft",
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    ...overrides,
  };
}

describe("recallMemory", () => {
  it("returns corrections matching task keywords", async () => {
    const store = mockCorrectionStore([
      makeCorrection({ id: "c1", rule: "use bun not npm", trigger: { keywords: ["package", "npm", "install"], fileGlobs: [] } }),
      makeCorrection({ id: "c2", rule: "prefer interface", trigger: { keywords: ["typescript", "type"], fileGlobs: [] } }),
    ]);
    const learnStore = mockLearningStore();

    const result = await recallMemory(store, learnStore, {
      taskDescription: "install npm packages for the project",
    });

    expect(result.corrections.some((c) => c.id === "c1")).toBe(true);
    expect(result.corrections.some((c) => c.id === "c2")).toBe(false);
  });

  it("always includes hard corrections", async () => {
    const store = mockCorrectionStore([
      makeCorrection({ id: "c1", rule: "critical rule", severity: "hard", trigger: { keywords: ["obscure"], fileGlobs: [] } }),
    ]);
    const learnStore = mockLearningStore();

    const result = await recallMemory(store, learnStore, {
      taskDescription: "something completely unrelated",
    });

    expect(result.corrections.some((c) => c.id === "c1")).toBe(true);
  });

  it("returns compiled learnings if available", async () => {
    const store = mockCorrectionStore();
    const learnStore = mockLearningStore();
    await learnStore.writeCompiled({
      compiledAt: "2026-04-05",
      summary: "key insights",
      rawCount: 5,
    });

    const result = await recallMemory(store, learnStore, {});

    expect(result.compiledLearnings?.summary).toBe("key insights");
  });
});
