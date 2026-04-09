import { describe, it, expect } from "bun:test";
import { mockCorrectionStore } from "../../helpers/mocks.js";
import { captureCorrection } from "@/usecases/memory-correct.usecase.js";

describe("captureCorrection", () => {
  it("creates a correction via the store", async () => {
    const store = mockCorrectionStore();
    const result = await captureCorrection(store, {
      rule: "use bun not npm",
      source: "used npm install",
      keywords: ["package", "npm"],
      fileGlobs: ["*.sh"],
      severity: "hard",
    });

    expect(result.rule).toBe("use bun not npm");
    expect(result.severity).toBe("hard");
    expect(result.trigger.keywords).toEqual(["package", "npm"]);

    const stored = await store.get(result.id);
    expect(stored).toEqual(result);
  });
});
