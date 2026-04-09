import { describe, expect, it } from "bun:test";
import { recommendWorkerFit } from "@/usecases/worker-fit-recommendation.usecase.js";
import type { Feature } from "@/features/mission/domain/mission-types.js";

function makeFeature(overrides: Partial<Feature>): Feature {
  return {
    id: "f1",
    missionId: "mission-1",
    milestoneId: "m1",
    status: "pending",
    title: "Feature",
    description: "Implement the requested change.",
    workerType: "implementation",
    verificationSteps: [],
    dependsOn: [],
    fulfills: [],
    createdAt: "2026-04-02T10:00:00.000Z",
    updatedAt: "2026-04-02T10:00:00.000Z",
    ...overrides,
  };
}

describe("recommendWorkerFit", () => {
  it("prefers ready implementation work for codex", () => {
    const features = [
      makeFeature({
        id: "f-ready",
        title: "Build CLI transport adapter",
        description: "Implement the CLI transport and parser for feature run.",
      }),
      makeFeature({
        id: "f-blocked",
        title: "Review orchestration design",
        description: "Design a complex orchestration policy.",
        dependsOn: ["f-ready"],
      }),
    ];

    const recommendation = recommendWorkerFit("codex", features);

    expect(recommendation.featureId).toBe("f-ready");
    expect(recommendation.reason.toLowerCase()).toContain("ready");
  });

  it("prefers deeper orchestration work for claude-code", () => {
    const features = [
      makeFeature({
        id: "f-simple",
        title: "Fix small copy issue",
        description: "Tidy a small wording issue in Mission Control.",
      }),
      makeFeature({
        id: "f-complex",
        title: "Design runtime recovery orchestration",
        description: "Review the supervision, retry, and recovery lifecycle across the mission.",
        verificationSteps: ["run build", "run tests", "check preview"],
      }),
    ];

    const recommendation = recommendWorkerFit("claude-code", features);

    expect(recommendation.featureId).toBe("f-complex");
    expect(recommendation.reason.toLowerCase()).toContain("complex");
  });

  it("falls back cleanly when no visible work matches", () => {
    const recommendation = recommendWorkerFit("gemini", [
      makeFeature({ id: "f-done", status: "done", title: "Completed task" }),
    ]);

    expect(recommendation.featureId).toBeUndefined();
    expect(recommendation.fallbackReason).toContain("No clear match");
  });
});
