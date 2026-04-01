import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsAssertionStoreAdapter } from "../../../src/adapters/assertion-store.adapter.js";
import { FsFeatureStoreAdapter } from "../../../src/adapters/feature-store.adapter.js";
import { FsMissionStoreAdapter } from "../../../src/adapters/mission-store.adapter.js";
import { FsRuntimeStoreAdapter } from "../../../src/adapters/runtime-store.adapter.js";
import type { MilestoneInput } from "../../../src/domain/mission-types.js";
import {
  recoverMissionRuntimeFailures,
  recoverRuntimeFailure,
} from "../../../src/usecases/runtime-recovery.usecase.js";

async function createSampleMission(
  missionStore: FsMissionStoreAdapter,
  featureStore: FsFeatureStoreAdapter,
  assertionStore: FsAssertionStoreAdapter,
): Promise<string> {
  const sampleMilestones: MilestoneInput[] = [
    { id: "m1", title: "Milestone 1", description: "First milestone", order: 0 },
  ];

  const samplePlan = {
    title: "Recovery Mission",
    description: "A mission for runtime recovery tests",
    milestones: sampleMilestones,
    features: [
      {
        id: "f1",
        milestoneId: "m1",
        title: "Feature 1",
        description: "First feature",
        workerType: "test-skill",
        verificationSteps: ["step1"],
        dependsOn: [],
      },
    ],
  };

  const { createMission } = await import("../../../src/usecases/mission-lifecycle.usecase.js");
  const result = await createMission(missionStore, featureStore, assertionStore, samplePlan);
  return result.mission.id;
}

describe("recoverRuntimeFailure", () => {
  let tmpDir: string;
  let missionStore: FsMissionStoreAdapter;
  let featureStore: FsFeatureStoreAdapter;
  let assertionStore: FsAssertionStoreAdapter;
  let runtimeStore: FsRuntimeStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "runtime-recovery-"));
    missionStore = new FsMissionStoreAdapter(tmpDir);
    featureStore = new FsFeatureStoreAdapter(tmpDir);
    assertionStore = new FsAssertionStoreAdapter(tmpDir);
    runtimeStore = new FsRuntimeStoreAdapter(tmpDir);
  });

  it("requeues active work when runtime has failed and budget remains", async () => {
    const missionId = await createSampleMission(missionStore, featureStore, assertionStore);
    await featureStore.update(missionId, "f1", { status: "assigned" });
    await runtimeStore.save(missionId, "f1", {
      featureId: "f1",
      attemptId: "attempt-1",
      attempt: 1,
      agent: "unknown",
      runtimeState: "live",
      startedAt: "2026-04-01T00:00:00.000Z",
      lastSeenAt: "2026-04-01T00:00:00.000Z",
      leaseExpiresAt: "2026-04-01T00:01:00.000Z",
      recoveryMetadata: {
        retryCount: 0,
        history: [],
      },
    });

    const result = await recoverRuntimeFailure(
      missionStore,
      featureStore,
      runtimeStore,
      missionId,
      "f1",
      Date.parse("2026-04-01T00:10:00.000Z"),
    );

    expect(result.recovered).toBe(true);
    expect(result.exhausted).toBe(false);
    expect(result.feature?.status).toBe("pending");
    expect(result.runtime).toMatchObject({
      runtimeState: "recoverable",
    });
    expect(result.runtime?.recoveryMetadata.retryCount).toBe(1);
  });

  it("does not recover inactive or semantic states", async () => {
    const missionId = await createSampleMission(missionStore, featureStore, assertionStore);
    await featureStore.update(missionId, "f1", { status: "blocked" });
    await runtimeStore.save(missionId, "f1", {
      featureId: "f1",
      attemptId: "attempt-1",
      attempt: 1,
      agent: "unknown",
      runtimeState: "failed",
      startedAt: "2026-04-01T00:00:00.000Z",
      lastSeenAt: "2026-04-01T00:00:00.000Z",
      leaseExpiresAt: "2026-04-01T00:01:00.000Z",
      recoveryMetadata: {
        retryCount: 0,
        history: [],
      },
    });

    const result = await recoverRuntimeFailure(
      missionStore,
      featureStore,
      runtimeStore,
      missionId,
      "f1",
      Date.parse("2026-04-01T00:10:00.000Z"),
    );

    expect(result.recovered).toBe(false);
    expect(result.feature?.status).toBe("blocked");
  });

    it("stops auto-retrying when the recovery budget is exhausted", async () => {
      const missionId = await createSampleMission(missionStore, featureStore, assertionStore);
      await featureStore.update(missionId, "f1", { status: "in-progress" });
    await runtimeStore.save(missionId, "f1", {
      featureId: "f1",
      attemptId: "attempt-1",
      attempt: 1,
      agent: "unknown",
      runtimeState: "failed",
      startedAt: "2026-04-01T00:00:00.000Z",
      lastSeenAt: "2026-04-01T00:00:00.000Z",
      leaseExpiresAt: "2026-04-01T00:01:00.000Z",
      recoveryMetadata: {
        retryCount: 2,
        history: [],
      },
    });

    const result = await recoverRuntimeFailure(
      missionStore,
      featureStore,
      runtimeStore,
      missionId,
      "f1",
      Date.parse("2026-04-01T00:10:00.000Z"),
    );

    expect(result.recovered).toBe(false);
      expect(result.exhausted).toBe(true);
      expect(result.feature?.status).toBe("in-progress");
      expect(result.runtime?.runtimeState).toBe("failed");
      expect(result.runtime?.recoveryMetadata.lastRecoveryReason).toContain("retry budget exhausted");
    });

    it("does not append duplicate exhausted audit entries when recovery runs twice", async () => {
      const missionId = await createSampleMission(missionStore, featureStore, assertionStore);
      await featureStore.update(missionId, "f1", { status: "in-progress" });
      await runtimeStore.save(missionId, "f1", {
        featureId: "f1",
        attemptId: "attempt-1",
        attempt: 1,
        agent: "unknown",
        runtimeState: "failed",
        startedAt: "2026-04-01T00:00:00.000Z",
        lastSeenAt: "2026-04-01T00:00:00.000Z",
        leaseExpiresAt: "2026-04-01T00:01:00.000Z",
        failureReason: "worker vanished",
        recoveryMetadata: {
          retryCount: 2,
          history: [],
        },
      });

      await recoverRuntimeFailure(
        missionStore,
        featureStore,
        runtimeStore,
        missionId,
        "f1",
        Date.parse("2026-04-01T00:10:00.000Z"),
      );
      await recoverRuntimeFailure(
        missionStore,
        featureStore,
        runtimeStore,
        missionId,
        "f1",
        Date.parse("2026-04-01T00:10:00.000Z"),
      );

      const runtime = await runtimeStore.get(missionId, "f1");
      expect(runtime?.recoveryMetadata.history).toHaveLength(1);
      expect(runtime?.recoveryMetadata.history[0]).toMatchObject({
        reason: "worker vanished",
        fromState: "failed",
        toState: "failed",
      });
      expect(runtime?.recoveryMetadata.lastRecoveryReason).toBe("worker vanished (retry budget exhausted)");
    });

    it("skips orphan runtime artifacts during mission-wide recovery", async () => {
      const missionId = await createSampleMission(missionStore, featureStore, assertionStore);
      await runtimeStore.save(missionId, "orphan-feature", {
        featureId: "orphan-feature",
        attemptId: "attempt-1",
        attempt: 1,
        agent: "unknown",
        runtimeState: "failed",
        startedAt: "2026-04-01T00:00:00.000Z",
        lastSeenAt: "2026-04-01T00:00:00.000Z",
        leaseExpiresAt: "2026-04-01T00:01:00.000Z",
        recoveryMetadata: {
          retryCount: 0,
          history: [],
        },
      });

      await expect(
        recoverMissionRuntimeFailures(
          missionStore,
          featureStore,
          runtimeStore,
          missionId,
          Date.parse("2026-04-01T00:10:00.000Z"),
        ),
      ).resolves.toEqual({ recovered: [] });
    });
  });
