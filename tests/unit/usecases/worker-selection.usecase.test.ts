import { describe, expect, it } from "bun:test";
import { selectWorker } from "../../../src/usecases/worker-selection.usecase.js";
import { DEFAULT_CONFIG } from "../../../src/domain/defaults.js";
import type { Feature } from "../../../src/domain/mission-types.js";

const feature: Feature = {
  id: "f1",
  missionId: "m1",
  milestoneId: "m1",
  status: "pending",
  title: "Feature",
  description: "desc",
  workerType: "test-skill",
  verificationSteps: [],
  dependsOn: [],
  fulfills: [],
  createdAt: "2026-04-02T10:00:00.000Z",
  updatedAt: "2026-04-02T10:00:00.000Z",
};

describe("selectWorker", () => {
  it("returns the configured default worker", () => {
    const result = selectWorker(DEFAULT_CONFIG, feature);
    expect(result.slug).toBe("codex");
  });

  it("rotates away from the last failed worker when configured", () => {
    const result = selectWorker(
      {
        ...DEFAULT_CONFIG,
        execution: {
          ...DEFAULT_CONFIG.execution,
          rotateWorkerOnRetry: true,
        },
      },
      feature,
      [{
        id: "attempt-1",
        missionId: "m1",
        featureId: "f1",
        worker: "codex",
        transport: "cli",
        attemptId: "attempt-1",
        startedAt: "2026-04-02T10:00:00.000Z",
        completedAt: "2026-04-02T10:00:05.000Z",
        durationMs: 5000,
        success: false,
        exitCode: 1,
        summary: "failed",
        stdoutRaw: "",
        stderrRaw: "",
        filesChanged: [],
      }],
    );

    expect(result.slug).not.toBe("codex");
  });
});
