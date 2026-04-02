import { beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runFeatures } from "../../../src/usecases/run-features.usecase.js";
import { DEFAULT_CONFIG } from "../../../src/domain/defaults.js";
import {
  mockAssertionStore,
  mockExecutionStore,
  mockFeatureStore,
  mockMissionStore,
  mockRuntimeStore,
  mockTransport,
} from "../../helpers/mocks.js";
import type { Feature, Mission } from "../../../src/domain/mission-types.js";

describe("runFeatures", () => {
  let baseDir: string;
  let mission: Mission;
  let features: Feature[];

  beforeEach(async () => {
    baseDir = await mkdtemp(join(tmpdir(), "run-features-"));
    await mkdir(join(baseDir, ".maestro", "skills", "test-skill"), { recursive: true });
    await writeFile(
      join(baseDir, ".maestro", "skills", "test-skill", "SKILL.md"),
      "# test skill\n",
    );

    mission = {
      id: "mission-1",
      status: "approved",
      title: "Mission",
      description: "desc",
      milestones: [{
        id: "m1",
        title: "Milestone",
        description: "desc",
        order: 0,
        featureIds: ["f1", "f2", "f3"],
      }],
      features: ["f1", "f2", "f3"],
      createdAt: "2026-04-02T10:00:00.000Z",
      updatedAt: "2026-04-02T10:00:00.000Z",
    };

    features = [
      {
        id: "f1",
        missionId: "mission-1",
        milestoneId: "m1",
        status: "pending",
        title: "First",
        description: "desc",
        workerType: "test-skill",
        verificationSteps: [],
        dependsOn: [],
        fulfills: [],
        createdAt: "2026-04-02T10:00:00.000Z",
        updatedAt: "2026-04-02T10:00:00.000Z",
      },
      {
        id: "f2",
        missionId: "mission-1",
        milestoneId: "m1",
        status: "pending",
        title: "Second",
        description: "desc",
        workerType: "test-skill",
        verificationSteps: [],
        dependsOn: ["f1"],
        fulfills: [],
        createdAt: "2026-04-02T10:00:00.000Z",
        updatedAt: "2026-04-02T10:00:00.000Z",
      },
      {
        id: "f3",
        missionId: "mission-1",
        milestoneId: "m1",
        status: "pending",
        title: "Third",
        description: "desc",
        workerType: "test-skill",
        verificationSteps: [],
        dependsOn: [],
        fulfills: [],
        createdAt: "2026-04-02T10:00:00.000Z",
        updatedAt: "2026-04-02T10:00:00.000Z",
      },
    ];
  });

  it("supports dry-run prompt generation without spawning", async () => {
    let spawnCount = 0;
    const result = await runFeatures(
      {
        missionStore: mockMissionStore([mission]),
        featureStore: mockFeatureStore(mission.id, features),
        assertionStore: mockAssertionStore(mission.id, []),
        runtimeStore: mockRuntimeStore(),
        executionStore: mockExecutionStore(),
        transport: mockTransport([], () => {
          spawnCount += 1;
        }),
        baseDir,
        config: DEFAULT_CONFIG,
      },
      {
        missionId: mission.id,
        dryRun: true,
      },
    );

    expect(result.success).toBe(true);
    expect(spawnCount).toBe(0);
    expect(result.outcomes.every((outcome) => outcome.status === "dry-run" || outcome.status === "skipped")).toBe(true);
  });

  it("runs ready features in order and skips blocked dependencies", async () => {
    const result = await runFeatures(
      {
        missionStore: mockMissionStore([mission]),
        featureStore: mockFeatureStore(mission.id, features),
        assertionStore: mockAssertionStore(mission.id, []),
        runtimeStore: mockRuntimeStore(),
        executionStore: mockExecutionStore(),
        transport: mockTransport([{
          success: true,
          exitCode: 0,
          summary: "ok",
          stdoutRaw: JSON.stringify({
            salientSummary: "ok",
            whatWasImplemented: "done",
            whatWasLeftUndone: "",
            verification: { commandsRun: [], interactiveChecks: [] },
            tests: { added: [] },
            discoveredIssues: [],
          }),
          stderrRaw: "",
          filesChanged: [],
          durationMs: 5,
          parsedOutput: JSON.stringify({
            salientSummary: "ok",
            whatWasImplemented: "done",
            whatWasLeftUndone: "",
            verification: { commandsRun: [], interactiveChecks: [] },
            tests: { added: [] },
            discoveredIssues: [],
          }),
        }, {
          success: true,
          exitCode: 0,
          summary: "ok2",
          stdoutRaw: "",
          stderrRaw: "",
          filesChanged: [],
          durationMs: 5,
        }, {
          success: true,
          exitCode: 0,
          summary: "ok3",
          stdoutRaw: "",
          stderrRaw: "",
          filesChanged: [],
          durationMs: 5,
        }]),
        baseDir,
        config: DEFAULT_CONFIG,
      },
      {
        missionId: mission.id,
      },
    );

    expect(result.success).toBe(true);
    expect(result.outcomes.filter((outcome) => outcome.status === "done")).toHaveLength(3);
    expect(result.outcomes[0]?.featureId).toBe("f1");
  });

  it("stops on first failure", async () => {
    const result = await runFeatures(
      {
        missionStore: mockMissionStore([mission]),
        featureStore: mockFeatureStore(mission.id, features),
        assertionStore: mockAssertionStore(mission.id, []),
        runtimeStore: mockRuntimeStore(),
        executionStore: mockExecutionStore(),
        transport: mockTransport([{
          success: false,
          exitCode: 1,
          summary: "boom",
          stdoutRaw: "",
          stderrRaw: "failed",
          filesChanged: [],
          durationMs: 5,
          failureClass: "worker-crash",
        }]),
        baseDir,
        config: DEFAULT_CONFIG,
      },
      {
        missionId: mission.id,
      },
    );

    expect(result.success).toBe(false);
    expect(result.stoppedOnFeatureId).toBe("f1");
    expect(result.outcomes[0]?.status).toBe("blocked");
  });

  it("fails fast when a feature already has live runtime ownership", async () => {
    await expect(runFeatures(
      {
        missionStore: mockMissionStore([mission]),
        featureStore: mockFeatureStore(mission.id, [features[0]!]),
        assertionStore: mockAssertionStore(mission.id, []),
        runtimeStore: mockRuntimeStore([{
          featureId: "f1",
          attemptId: "attempt-1",
          attempt: 1,
          agent: "codex",
          runtimeState: "live",
          startedAt: "2026-04-02T10:00:00.000Z",
          lastSeenAt: new Date().toISOString(),
          leaseExpiresAt: new Date(Date.now() + 60_000).toISOString(),
          recoveryMetadata: {
            retryCount: 0,
            history: [],
          },
        }]),
        executionStore: mockExecutionStore(),
        transport: mockTransport(),
        baseDir,
        config: DEFAULT_CONFIG,
      },
      {
        missionId: mission.id,
      },
    )).rejects.toThrow("live runtime ownership");
  });
});
