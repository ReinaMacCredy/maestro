import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsMissionStoreAdapter, FsFeatureStoreAdapter, FsAssertionStoreAdapter } from "@/shared/domain/legacy-mission";
import { createMission } from "@/features/mission/usecases/mission-lifecycle.usecase.js";
import { updateFeature } from "@/features/mission/feature/usecases/feature-lifecycle.usecase.js";
import { FsReplyStoreAdapter } from "@/features/reply/adapters/fs-reply-store.adapter.js";
import { writeAgentReply } from "@/features/reply/usecases/write-reply.usecase.js";
import { ingestReply } from "@/features/reply/usecases/ingest-reply.usecase.js";

let tmpDir: string;
let missionStore: FsMissionStoreAdapter;
let featureStore: FsFeatureStoreAdapter;
let assertionStore: FsAssertionStoreAdapter;
let replyStore: FsReplyStoreAdapter;

async function setupMission(): Promise<{ missionId: string; featureId: string }> {
  const plan = {
    title: "Reply Ingest Fixture",
    description: "A tiny mission for ingest tests",
    milestones: [{ id: "m1", title: "M1", description: "desc", order: 0 }],
    features: [
      {
        id: "f1",
        milestoneId: "m1",
        title: "Feature 1",
        description: "first",
        agentType: "test-skill",
        verificationSteps: ["step 1"],
        dependsOn: [],
        fulfills: ["assertion1"],
      },
    ],
  };
  const result = await createMission(missionStore, featureStore, assertionStore, plan);
  return { missionId: result.mission.id, featureId: "f1" };
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-ingest-reply-"));
  missionStore = new FsMissionStoreAdapter(tmpDir);
  featureStore = new FsFeatureStoreAdapter(tmpDir);
  assertionStore = new FsAssertionStoreAdapter(tmpDir);
  replyStore = new FsReplyStoreAdapter(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("ingestReply", () => {
  it("returns undefined when no reply exists on disk", async () => {
    const { missionId, featureId } = await setupMission();

    const result = await ingestReply(
      { missionStore, featureStore, assertionStore, replyStore, baseDir: tmpDir },
      missionId,
      featureId,
    );

    expect(result).toBeUndefined();
  });

  it("advances review -> done when reply is completed and assertions pass", async () => {
    const { missionId, featureId } = await setupMission();
    // Walk feature through pending -> in-progress -> review
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "in-progress" });
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "review" });
    // Mark assertions passed
    const assertions = await assertionStore.list(missionId);
    for (const a of assertions) {
      await assertionStore.update(missionId, a.id, { result: "passed" });
    }

    await writeAgentReply(replyStore, {
      missionId,
      featureId,
      outcome: "completed",
      notes: "all green",
    });

    const result = await ingestReply(
      { missionStore, featureStore, assertionStore, replyStore, baseDir: tmpDir },
      missionId,
      featureId,
    );

    expect(result).toBeDefined();
    expect(result?.featureAdvanced).toBe(true);
    expect(result?.kickedBack).toBe(false);
    const after = await featureStore.get(missionId, featureId);
    expect(after?.status).toBe("done");
  });

  it("downgrades completed to kicked-back when assertions are not all passed", async () => {
    const { missionId, featureId } = await setupMission();
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "in-progress" });
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "review" });
    // Assertions left pending

    await writeAgentReply(replyStore, {
      missionId,
      featureId,
      outcome: "completed",
    });

    const result = await ingestReply(
      { missionStore, featureStore, assertionStore, replyStore, baseDir: tmpDir },
      missionId,
      featureId,
    );

    expect(result?.kickedBack).toBe(true);
    expect(result?.downgradeReason).toContain("assertions");
    const after = await featureStore.get(missionId, featureId);
    expect(after?.status).toBe("pending"); // kicked-back target
  });

  it("downgrades completed to kicked-back when feature is not in review", async () => {
    const { missionId, featureId } = await setupMission();
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "in-progress" });

    await writeAgentReply(replyStore, {
      missionId,
      featureId,
      outcome: "completed",
    });

    const result = await ingestReply(
      { missionStore, featureStore, assertionStore, replyStore, baseDir: tmpDir },
      missionId,
      featureId,
    );

    expect(result?.kickedBack).toBe(true);
    expect(result?.downgradeReason).toContain("review");
    // in-progress -> pending is not a valid transition; state unchanged.
    const after = await featureStore.get(missionId, featureId);
    expect(after?.status).toBe("in-progress");
  });

  it("kicks back review -> pending with retryReason on kicked-back reply", async () => {
    const { missionId, featureId } = await setupMission();
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "in-progress" });
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "review" });

    await writeAgentReply(replyStore, {
      missionId,
      featureId,
      outcome: "kicked-back",
      notes: "broke migration",
    });

    await ingestReply(
      { missionStore, featureStore, assertionStore, replyStore, baseDir: tmpDir },
      missionId,
      featureId,
    );

    const after = await featureStore.get(missionId, featureId);
    expect(after?.status).toBe("pending");
  });

  it("moves review -> blocked on abandoned reply", async () => {
    const { missionId, featureId } = await setupMission();
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "in-progress" });
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "review" });

    await writeAgentReply(replyStore, {
      missionId,
      featureId,
      outcome: "abandoned",
      notes: "out of scope",
    });

    await ingestReply(
      { missionStore, featureStore, assertionStore, replyStore, baseDir: tmpDir },
      missionId,
      featureId,
    );

    const after = await featureStore.get(missionId, featureId);
    expect(after?.status).toBe("blocked");
  });

  it("is idempotent: second ingest is a no-op", async () => {
    const { missionId, featureId } = await setupMission();
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "in-progress" });
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "review" });
    const assertions = await assertionStore.list(missionId);
    for (const a of assertions) await assertionStore.update(missionId, a.id, { result: "passed" });

    await writeAgentReply(replyStore, { missionId, featureId, outcome: "completed" });

    const first = await ingestReply(
      { missionStore, featureStore, assertionStore, replyStore, baseDir: tmpDir },
      missionId,
      featureId,
    );
    const second = await ingestReply(
      { missionStore, featureStore, assertionStore, replyStore, baseDir: tmpDir },
      missionId,
      featureId,
    );

    expect(first?.featureAdvanced).toBe(true);
    expect(second?.featureAdvanced).toBe(false);
    expect(second?.reply).toEqual(first!.reply);
  });

  it("invokes recordPrincipleOutcomes hook with inferred outcome", async () => {
    const { missionId, featureId } = await setupMission();
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "in-progress" });
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "review" });
    const assertions = await assertionStore.list(missionId);
    for (const a of assertions) await assertionStore.update(missionId, a.id, { result: "passed" });

    await writeAgentReply(replyStore, { missionId, featureId, outcome: "completed" });

    const calls: Array<{ featureId: string; outcome: string }> = [];
    const result = await ingestReply(
      {
        missionStore,
        featureStore,
        assertionStore,
        replyStore,
        baseDir: tmpDir,
        recordPrincipleOutcomes: async (fid, o) => {
          calls.push({ featureId: fid, outcome: o });
          return { recorded: 3, complete: true };
        },
      },
      missionId,
      featureId,
    );

    expect(calls).toEqual([{ featureId, outcome: "completed" }]);
    expect(result?.principlesRecorded).toBe(3);
  });

  it("retries principle outcome recording without re-downgrading a completed reply", async () => {
    const { missionId, featureId } = await setupMission();
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "in-progress" });
    await updateFeature(missionStore, featureStore, tmpDir, missionId, featureId, { status: "review" });
    const assertions = await assertionStore.list(missionId);
    for (const assertion of assertions) {
      await assertionStore.update(missionId, assertion.id, { result: "passed" });
    }

    await writeAgentReply(replyStore, {
      missionId,
      featureId,
      outcome: "completed",
    });

    const first = await ingestReply(
      {
        missionStore,
        featureStore,
        assertionStore,
        replyStore,
        baseDir: tmpDir,
        recordPrincipleOutcomes: async () => ({ recorded: 0, complete: false }),
      },
      missionId,
      featureId,
    );
    expect(first?.featureAdvanced).toBe(true);
    expect(first?.kickedBack).toBe(false);
    expect(await replyStore.isIngested(missionId, featureId)).toBe(false);
    expect((await featureStore.get(missionId, featureId))?.status).toBe("done");

    const second = await ingestReply(
      {
        missionStore,
        featureStore,
        assertionStore,
        replyStore,
        baseDir: tmpDir,
        recordPrincipleOutcomes: async () => ({ recorded: 2, complete: true }),
      },
      missionId,
      featureId,
    );
    expect(second?.featureAdvanced).toBe(false);
    expect(second?.kickedBack).toBe(false);
    expect(second?.principlesRecorded).toBe(2);
    expect(await replyStore.isIngested(missionId, featureId)).toBe(true);
    expect((await featureStore.get(missionId, featureId))?.status).toBe("done");
  });

  it("records reply but does not move state when feature is missing", async () => {
    const { missionId } = await setupMission();
    await writeAgentReply(replyStore, {
      missionId,
      featureId: "f-missing",
      outcome: "completed",
    });

    const result = await ingestReply(
      { missionStore, featureStore, assertionStore, replyStore, baseDir: tmpDir },
      missionId,
      "f-missing",
    );

    expect(result).toBeDefined();
    expect(result?.featureAdvanced).toBe(false);
    expect(result?.downgradeReason).toContain("not found");
    expect(await replyStore.isIngested(missionId, "f-missing")).toBe(true);
  });

  it("does not ingest a reply written for another mission with the same feature id", async () => {
    const first = await setupMission();
    const second = await setupMission();

    await updateFeature(missionStore, featureStore, tmpDir, second.missionId, second.featureId, { status: "in-progress" });
    await updateFeature(missionStore, featureStore, tmpDir, second.missionId, second.featureId, { status: "review" });
    const assertions = await assertionStore.list(second.missionId);
    for (const assertion of assertions) {
      await assertionStore.update(second.missionId, assertion.id, { result: "passed" });
    }

    await writeAgentReply(replyStore, {
      missionId: second.missionId,
      featureId: second.featureId,
      outcome: "completed",
    });

    const firstResult = await ingestReply(
      { missionStore, featureStore, assertionStore, replyStore, baseDir: tmpDir },
      first.missionId,
      first.featureId,
    );
    expect(firstResult).toBeUndefined();
    expect(await replyStore.isIngested(first.missionId, first.featureId)).toBe(false);

    const secondResult = await ingestReply(
      { missionStore, featureStore, assertionStore, replyStore, baseDir: tmpDir },
      second.missionId,
      second.featureId,
    );
    expect(secondResult?.featureAdvanced).toBe(true);
  });
});
