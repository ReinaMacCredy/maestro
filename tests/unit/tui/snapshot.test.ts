import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { buildHomeSnapshot, buildSnapshot, type SnapshotDeps } from "../../../src/tui/state/snapshot.js";
import { FsMissionStoreAdapter } from "../../../src/adapters/mission-store.adapter.js";
import { FsFeatureStoreAdapter } from "../../../src/adapters/feature-store.adapter.js";
import { FsAssertionStoreAdapter } from "../../../src/adapters/assertion-store.adapter.js";
import { FsCheckpointStoreAdapter } from "../../../src/adapters/checkpoint-store.adapter.js";
import { FsRuntimeStoreAdapter } from "../../../src/adapters/runtime-store.adapter.js";
import { FsRuntimeEventStoreAdapter } from "../../../src/adapters/runtime-event-store.adapter.js";
import type { WorkerRuntime } from "../../../src/domain/runtime-types.js";
import type { CassPort } from "../../../src/ports/cass.port.js";
import type { ConfigPort } from "../../../src/ports/config.port.js";
import type { GitPort } from "../../../src/ports/git.port.js";
import type { HandoffStorePort } from "../../../src/ports/handoff-store.port.js";

let tmpDir: string;
let deps: SnapshotDeps;
let runtimeStore: FsRuntimeStoreAdapter;
let runtimeEventStore: FsRuntimeEventStoreAdapter;

async function initGitRepo(cwd: string): Promise<void> {
  const proc = Bun.spawn(["git", "init", "-b", "main"], { cwd, stdout: "pipe", stderr: "pipe" });
  await proc.exited;
}

const CLI = ["bun", "run", join(import.meta.dir, "..", "..", "..", "src", "index.ts")];

async function run(args: string[], cwd: string): Promise<{ stdout: string; exitCode: number }> {
  const proc = Bun.spawn([...CLI, ...args], { stdout: "pipe", stderr: "pipe", cwd });
  const stdout = await new Response(proc.stdout).text();
  const exitCode = await proc.exited;
  return { stdout: stdout.trim(), exitCode };
}

function createSamplePlan(): object {
  return {
    title: "Snapshot Test Mission",
    description: "Testing snapshot assembly",
    milestones: [
      { id: "m1", title: "Milestone 1", description: "First", order: 0 },
      { id: "m2", title: "Milestone 2", description: "Second", order: 1 },
    ],
    features: [
      { id: "f1", milestoneId: "m1", title: "Feature 1", description: "First feature", workerType: "test", verificationSteps: ["check it"], fulfills: ["a-f1-1"] },
      { id: "f2", milestoneId: "m1", title: "Feature 2", description: "Second feature", workerType: "test", verificationSteps: ["verify"], dependsOn: ["f1"] },
      { id: "f3", milestoneId: "m2", title: "Feature 3", description: "Third feature", workerType: "test", verificationSteps: ["verify"] },
    ],
  };
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-snapshot-"));
  await initGitRepo(tmpDir);
  runtimeStore = new FsRuntimeStoreAdapter(tmpDir);
  runtimeEventStore = new FsRuntimeEventStoreAdapter(tmpDir);
  deps = {
    missionStore: new FsMissionStoreAdapter(tmpDir),
    featureStore: new FsFeatureStoreAdapter(tmpDir),
    assertionStore: new FsAssertionStoreAdapter(tmpDir),
    checkpointStore: new FsCheckpointStoreAdapter(tmpDir),
    handoffStore: {
      create: async () => "handoff-id",
      get: async () => undefined,
      getLatestPending: async () => undefined,
      listIds: async () => [],
      list: async () => [],
      updateStatus: async () => undefined,
      delete: async () => undefined,
    } satisfies HandoffStorePort,
    config: {
      load: async () => ({ defaultAgent: "codex" }),
      loadLayers: async () => ({
        defaults: { defaultAgent: "codex" },
        effective: { defaultAgent: "codex" },
        errors: [],
        paths: {
          project: join(tmpDir, ".maestro", "config.yaml"),
          global: join(tmpDir, "..", ".maestro", "config.yaml"),
        },
      }),
      write: async () => undefined,
      exists: async () => true,
    } satisfies ConfigPort,
    cass: {
      isAvailable: async () => true,
      hasBinary: async () => true,
      indexOnce: async () => undefined,
      search: async () => ({ query: "", hits: [], count: 0, totalMatches: 0 }),
    } satisfies CassPort,
    git: {
      getState: async () => ({
        branch: "main",
        recentCommits: [],
        changedFiles: ["src/db.ts", "src/config.ts", "tests/db.test.ts"],
        workingTreeClean: false,
        diffStat: "+42 -11",
      }),
      isRepo: async () => true,
    } satisfies GitPort,
    runtimeStore,
    runtimeEventStore,
    cwd: tmpDir,
  };
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("buildSnapshot", () => {
  it("returns correct featureProgress counts", async () => {
    const plan = createSamplePlan();
    await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
    const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id;

    const snapshot = await buildSnapshot(deps, missionId);

    expect(snapshot.featureProgress.total).toBe(3);
    expect(snapshot.featureProgress.done).toBe(0);
    expect(snapshot.featureProgress.active).toBe(0);
    expect(snapshot.session?.branch).toBe("main");
    expect(snapshot.configSummary?.missionDirectory).toBe(`.maestro/missions/${missionId}`);
    expect(snapshot.pendingHandoffs).toEqual([]);
  }, 15_000);

  it("derives a dedicated statusProgress summary for the top strip", async () => {
    const plan = createSamplePlan();
    await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
    const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id;

    await run(["mission", "approve", missionId, "--json"], tmpDir);
    await run(["feature", "update", "f1", "--mission", missionId, "--status", "in-progress", "--json"], tmpDir);
    await run(["feature", "update", "f1", "--mission", missionId, "--status", "review", "--json"], tmpDir);
    await run(["feature", "update", "f1", "--mission", missionId, "--status", "done", "--json"], tmpDir);
    await run(["feature", "update", "f2", "--mission", missionId, "--status", "assigned", "--json"], tmpDir);
    await run(["feature", "update", "f3", "--mission", missionId, "--status", "in-progress", "--json"], tmpDir);
    await run(["feature", "update", "f3", "--mission", missionId, "--status", "review", "--json"], tmpDir);
    await run(["feature", "update", "f3", "--mission", missionId, "--status", "blocked", "--json"], tmpDir);

    const snapshot = await buildSnapshot(deps, missionId);

    expect(snapshot.statusProgress).toEqual({
      completed: 1,
      total: 3,
      inFlight: 1,
      blocked: 1,
      queued: 0,
      completionPct: 33,
    });
    expect(snapshot.runtimeProcesses.map((process) => process.featureId)).toEqual(["f2"]);
    expect(snapshot.runtimeProcesses.find((process) => process.featureId === "f2")?.isLive).toBe(true);
  }, 15_000);

  it("projects live runtime output metadata from persisted runtime events", async () => {
    const plan = createSamplePlan();
    await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
    const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id;

    await run(["mission", "approve", missionId, "--json"], tmpDir);
    await run(["feature", "update", "f1", "--mission", missionId, "--status", "assigned", "--json"], tmpDir);

    const runtime: WorkerRuntime = {
      featureId: "f1",
      attemptId: "attempt-1",
      attempt: 1,
      agent: "codex",
      runtimeState: "live",
      startedAt: new Date(Date.now() - 20_000).toISOString(),
      lastSeenAt: new Date(Date.now() - 2_000).toISOString(),
      leaseExpiresAt: new Date(Date.now() + 60_000).toISOString(),
      sessionId: "session-123",
      recoveryMetadata: {
        retryCount: 0,
        history: [],
      },
    };
    await runtimeStore.save(missionId, "f1", runtime);
    await runtimeEventStore.append(missionId, {
      id: "evt-1",
      missionId,
      featureId: "f1",
      attemptId: "attempt-1",
      worker: "codex",
      timestamp: new Date(Date.now() - 3_000).toISOString(),
      kind: "stdout",
      text: "Reading runtime-supervision.usecase.ts",
      sessionId: "session-123",
      runtimeState: "live",
    });

    const snapshot = await buildSnapshot(deps, missionId);

    expect(snapshot.activeWorker?.currentActivity).toContain("Reading runtime-supervision");
    expect(snapshot.activeWorker?.lastOutputAgeMs).toBeGreaterThanOrEqual(0);
    expect(snapshot.runtimeProcesses[0]?.outputLines?.[0]?.text).toContain("Reading runtime-supervision");
    expect(snapshot.progressLog.some((event) => event.kind === "worker")).toBe(true);
  }, 15_000);

  it("activeFeature matches first non-done feature", async () => {
    const plan = createSamplePlan();
    await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
    const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id;

    const snapshot = await buildSnapshot(deps, missionId);

    // All features are pending, so first pending feature is active
    expect(snapshot.activeFeature).not.toBeNull();
    expect(snapshot.activeFeature!.id).toBe("f1");
  }, 15_000);

  it("canPause is false for draft mission", async () => {
    const plan = createSamplePlan();
    await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
    const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id;

    const snapshot = await buildSnapshot(deps, missionId);

    expect(snapshot.canPause).toBe(false);
    expect(snapshot.canResume).toBe(false);
  }, 15_000);

  it("tokenCounters is null", async () => {
    const plan = createSamplePlan();
    await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
    const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id;

    const snapshot = await buildSnapshot(deps, missionId);
    expect(snapshot.tokenCounters).toBeNull();
  }, 15_000);

  it("milestones sorted by order", async () => {
    const plan = createSamplePlan();
    await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
    const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id;

    const snapshot = await buildSnapshot(deps, missionId);

    expect(snapshot.milestones.length).toBe(2);
    expect(snapshot.milestones[0]!.id).toBe("m1");
    expect(snapshot.milestones[1]!.id).toBe("m2");
    expect(snapshot.milestones[0]!.kind).toBe("work");
    expect(snapshot.milestones[0]!.profile).toBe("custom");
  }, 15_000);

  it("projects active gate metadata when a gate milestone is blocked", async () => {
    const plan = {
      title: "Gate Mission",
      description: "Gate test",
      milestones: [
        { id: "m1", title: "Plan Review", description: "Review", order: 0, kind: "gate", profile: "plan-review" },
      ],
      features: [
        { id: "f1", milestoneId: "m1", title: "Review", description: "Review work", workerType: "test", verificationSteps: ["check"] },
      ],
    };
    await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
    const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id;

    await run(["mission", "approve", missionId, "--json"], tmpDir);
    await run(["feature", "update", "f1", "--mission", missionId, "--status", "in-progress", "--json"], tmpDir);
    await run(["feature", "update", "f1", "--mission", missionId, "--status", "review", "--json"], tmpDir);
    await run(["feature", "update", "f1", "--mission", missionId, "--status", "blocked", "--json"], tmpDir);

    const snapshot = await buildSnapshot(deps, missionId);

    expect(snapshot.gateBlocked).toBe(true);
    expect(snapshot.gateLabel).toBe("Plan Review");
    expect(snapshot.milestones[0]).toMatchObject({
      id: "m1",
      kind: "gate",
      profile: "plan-review",
      status: "executing",
    });
  }, 15_000);

  it("progressLog contains at least mission created event", async () => {
    const plan = createSamplePlan();
    await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
    const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id;

    const snapshot = await buildSnapshot(deps, missionId);

    expect(snapshot.progressLog.length).toBeGreaterThan(0);
    const created = snapshot.progressLog.find((e) => e.title === "Mission created");
    expect(created).toBeDefined();
  }, 15_000);

  it("features includes all mission features", async () => {
    const plan = createSamplePlan();
    await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
    const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id;

    const snapshot = await buildSnapshot(deps, missionId);

    expect(snapshot.features.length).toBe(3);
    expect(snapshot.features.map((f) => f.id)).toEqual(["f1", "f2", "f3"]);
  }, 15_000);

  it("uses the fast CASS binary check for mission-control snapshots", async () => {
    const plan = createSamplePlan();
    await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
    const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id;

    let isAvailableCalls = 0;
    let hasBinaryCalls = 0;
    deps = {
      ...deps,
      cass: {
        isAvailable: async () => {
          isAvailableCalls += 1;
          return true;
        },
        hasBinary: async () => {
          hasBinaryCalls += 1;
          return true;
        },
        indexOnce: async () => undefined,
        search: async () => ({ query: "", hits: [], count: 0, totalMatches: 0 }),
      } satisfies CassPort,
    };

    const snapshot = await buildSnapshot(deps, missionId);

    expect(snapshot.configSummary?.cassAvailable).toBe(true);
    expect(isAvailableCalls).toBe(0);
    expect(hasBinaryCalls).toBe(1);
  }, 15_000);

  it("uses the fast CASS binary check for home snapshots", async () => {
    let isAvailableCalls = 0;
    let hasBinaryCalls = 0;
    const homeDeps = {
      handoffStore: deps.handoffStore,
      config: deps.config,
      cass: {
        isAvailable: async () => {
          isAvailableCalls += 1;
          return true;
        },
        hasBinary: async () => {
          hasBinaryCalls += 1;
          return true;
        },
        indexOnce: async () => undefined,
        search: async () => ({ query: "", hits: [], count: 0, totalMatches: 0 }),
      } satisfies CassPort,
      git: deps.git,
    };

    const snapshot = await buildHomeSnapshot(homeDeps, tmpDir);

    expect(snapshot.mode).toBe("home");
      expect(snapshot.configSummary?.cassAvailable).toBe(true);
      expect(isAvailableCalls).toBe(0);
      expect(hasBinaryCalls).toBe(1);
    }, 15_000);

    it("prefers explicit runtime state when runtime.json exists", async () => {
      const plan = createSamplePlan();
      await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
    const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id;

    await run(["mission", "approve", missionId, "--json"], tmpDir);
    await run(["feature", "update", "f1", "--mission", missionId, "--status", "assigned", "--json"], tmpDir);

    const runtime: WorkerRuntime = {
      featureId: "f1",
      attemptId: "attempt-1",
      attempt: 1,
      agent: "unknown",
      runtimeState: "starting",
      startedAt: new Date(Date.now() - 20_000).toISOString(),
      lastSeenAt: new Date(Date.now() - 5_000).toISOString(),
      leaseExpiresAt: new Date(Date.now() + 60_000).toISOString(),
      recoveryMetadata: {
        retryCount: 1,
        history: [],
      },
    };
    await runtimeStore.save(missionId, "f1", runtime);

    const snapshot = await buildSnapshot(deps, missionId);

    expect(snapshot.activeWorker).toMatchObject({
      featureId: "f1",
      runtimeState: "live",
      retryCount: 1,
    });
      expect(snapshot.runtimeProcesses[0]).toMatchObject({
        featureId: "f1",
        runtimeState: "live",
        retryCount: 1,
      });
    }, 15_000);

    it("projects failed runtime state without mutating feature or runtime storage", async () => {
      const plan = createSamplePlan();
      await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
      const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
      const missionId = JSON.parse(stdout).mission.id;

      await run(["mission", "approve", missionId, "--json"], tmpDir);
      await run(["feature", "update", "f1", "--mission", missionId, "--status", "assigned", "--json"], tmpDir);

      await runtimeStore.save(missionId, "f1", {
        featureId: "f1",
        attemptId: "attempt-failed-view",
        attempt: 1,
        agent: "unknown",
        runtimeState: "live",
        startedAt: new Date(Date.now() - 360_000).toISOString(),
        lastSeenAt: new Date(Date.now() - 360_000).toISOString(),
        leaseExpiresAt: new Date(Date.now() - 300_000).toISOString(),
        failureReason: "worker vanished",
        recoveryMetadata: {
          retryCount: 0,
          history: [],
        },
      });

      const snapshot = await buildSnapshot(deps, missionId);
      const feature = await deps.featureStore.get(missionId, "f1");
      const runtime = await runtimeStore.get(missionId, "f1");

      expect(snapshot.activeWorker).toMatchObject({
        featureId: "f1",
        runtimeState: "failed",
        failureReason: "worker vanished",
      });
      expect(feature?.status).toBe("assigned");
      expect(runtime).toMatchObject({
        runtimeState: "live",
        failureReason: "worker vanished",
      });
      expect(runtime?.recoveryMetadata.retryCount).toBe(0);
      expect(runtime?.recoveryMetadata.history).toHaveLength(0);
    }, 15_000);

    it("classifies stale and failed runtimes from heartbeat age", async () => {
    const plan = createSamplePlan();
    await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
    const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id;

      await run(["mission", "approve", missionId, "--json"], tmpDir);
      await run(["feature", "update", "f1", "--mission", missionId, "--status", "assigned", "--json"], tmpDir);
      await run(["feature", "update", "f2", "--mission", missionId, "--status", "assigned", "--json"], tmpDir);
      await run(["feature", "update", "f2", "--mission", missionId, "--status", "in-progress", "--json"], tmpDir);
      await run(["feature", "update", "f2", "--mission", missionId, "--status", "review", "--json"], tmpDir);

    await runtimeStore.save(missionId, "f1", {
      featureId: "f1",
      attemptId: "attempt-stale",
      attempt: 1,
      agent: "unknown",
      runtimeState: "live",
      startedAt: new Date(Date.now() - 120_000).toISOString(),
      lastSeenAt: new Date(Date.now() - 120_000).toISOString(),
      leaseExpiresAt: new Date(Date.now() - 30_000).toISOString(),
      recoveryMetadata: {
        retryCount: 0,
        history: [],
      },
    });
    await runtimeStore.save(missionId, "f2", {
      featureId: "f2",
      attemptId: "attempt-failed",
      attempt: 1,
      agent: "unknown",
      runtimeState: "live",
      startedAt: new Date(Date.now() - 360_000).toISOString(),
      lastSeenAt: new Date(Date.now() - 360_000).toISOString(),
      leaseExpiresAt: new Date(Date.now() - 300_000).toISOString(),
      failureReason: "worker vanished",
      recoveryMetadata: {
        retryCount: 0,
        history: [],
      },
    });

    const snapshot = await buildSnapshot(deps, missionId);

    expect(snapshot.runtimeProcesses.find((process) => process.featureId === "f1")?.runtimeState).toBe("stale");
      expect(snapshot.runtimeProcesses.find((process) => process.featureId === "f2")?.runtimeState).toBe("failed");
    }, 15_000);

    it("preserves explicit failed runtime state even when the lease is still active", async () => {
      const plan = createSamplePlan();
      await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
      const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
      const missionId = JSON.parse(stdout).mission.id;

      await run(["mission", "approve", missionId, "--json"], tmpDir);
      await run(["feature", "update", "f1", "--mission", missionId, "--status", "assigned", "--json"], tmpDir);
      await run(["feature", "update", "f1", "--mission", missionId, "--status", "in-progress", "--json"], tmpDir);
      await run(["feature", "update", "f1", "--mission", missionId, "--status", "review", "--json"], tmpDir);

      await runtimeStore.save(missionId, "f1", {
        featureId: "f1",
        attemptId: "attempt-explicit-failed",
        attempt: 1,
        agent: "unknown",
        runtimeState: "failed",
        startedAt: new Date(Date.now() - 20_000).toISOString(),
        lastSeenAt: new Date(Date.now() - 5_000).toISOString(),
        leaseExpiresAt: new Date(Date.now() + 60_000).toISOString(),
        failureReason: "worker exited",
        recoveryMetadata: {
          retryCount: 0,
          history: [],
        },
      });

      const snapshot = await buildSnapshot(deps, missionId);

      expect(snapshot.activeFeature).toMatchObject({
        id: "f1",
        runtimeState: "failed",
        failureReason: "worker exited",
      });
      expect(snapshot.runtimeProcesses.find((process) => process.featureId === "f1")).toMatchObject({
        featureId: "f1",
        runtimeState: "failed",
        failureReason: "worker exited",
      });
    }, 15_000);

    it("does not surface pending features with starting runtimes as live processes", async () => {
      const plan = createSamplePlan();
      await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
      const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
      const missionId = JSON.parse(stdout).mission.id;

      await runtimeStore.save(missionId, "f1", {
        featureId: "f1",
        attemptId: "attempt-starting",
        attempt: 1,
        agent: "unknown",
        runtimeState: "starting",
        startedAt: new Date(Date.now() - 5_000).toISOString(),
        lastSeenAt: new Date(Date.now() - 5_000).toISOString(),
        leaseExpiresAt: new Date(Date.now() + 60_000).toISOString(),
        recoveryMetadata: {
          retryCount: 0,
          history: [],
        },
      });

      const snapshot = await buildSnapshot(deps, missionId);

      expect(snapshot.activeFeature).toMatchObject({
        id: "f1",
        status: "pending",
      });
      expect(snapshot.activeFeature?.runtimeState).toBe("starting");
      expect(snapshot.runtimeProcesses.find((process) => process.featureId === "f1")).toBeUndefined();
    }, 15_000);

      it("keeps overview dependency chains visible when downstream work is linked but not blocked", async () => {
        const plan = createSamplePlan();
        await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
        const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
        const missionId = JSON.parse(stdout).mission.id;

      await run(["mission", "approve", missionId, "--json"], tmpDir);
      await run(["feature", "update", "f1", "--mission", missionId, "--status", "done", "--json"], tmpDir);
      await run(["feature", "update", "f2", "--mission", missionId, "--status", "assigned", "--json"], tmpDir);

      const snapshot = await buildSnapshot(deps, missionId);

        expect(snapshot.missionOverview?.dependencyMap).toEqual(
          expect.arrayContaining([
            expect.objectContaining({
              root: expect.objectContaining({ id: "f1" }),
              primaryDependent: expect.objectContaining({ id: "f2", status: "assigned" }),
              hiddenDependentCount: 0,
            }),
          ]),
        );
      }, 15_000);

      it("keeps review-only runtime ownership in the session sidebar and runtime detail rows", async () => {
        const plan = {
          title: "Review Ownership Mission",
          description: "Review runtime coverage",
          milestones: [
            { id: "m1", title: "Implementation", description: "Build", order: 0, kind: "work", profile: "implementation" },
            { id: "m2", title: "Code Review", description: "Review", order: 1, kind: "gate", profile: "code-review" },
          ],
          features: [
            { id: "f1", milestoneId: "m1", title: "Build auth", description: "Build auth", workerType: "backend", verificationSteps: ["check"] },
            { id: "f2", milestoneId: "m2", title: "Review auth", description: "Review auth", workerType: "reviewer", verificationSteps: ["inspect"] },
          ],
        };
        await writeFile(join(tmpDir, "plan.json"), JSON.stringify(plan));
        const { stdout } = await run(["mission", "create", "--file", join(tmpDir, "plan.json"), "--json"], tmpDir);
        const missionId = JSON.parse(stdout).mission.id;

        await run(["mission", "approve", missionId, "--json"], tmpDir);
        await run(["feature", "update", "f1", "--mission", missionId, "--status", "in-progress", "--json"], tmpDir);
        await run(["feature", "update", "f1", "--mission", missionId, "--status", "review", "--json"], tmpDir);
        await run(["feature", "update", "f1", "--mission", missionId, "--status", "done", "--json"], tmpDir);
        await run(["feature", "update", "f2", "--mission", missionId, "--status", "assigned", "--json"], tmpDir);
        await run(["feature", "update", "f2", "--mission", missionId, "--status", "in-progress", "--json"], tmpDir);
        await run(["feature", "update", "f2", "--mission", missionId, "--status", "review", "--json"], tmpDir);

        await runtimeStore.save(missionId, "f2", {
          featureId: "f2",
          attemptId: "attempt-review-owner",
          attempt: 1,
          agent: "codex",
          sessionId: "session-review-1234567890",
          runtimeState: "live",
          startedAt: new Date(Date.now() - 20_000).toISOString(),
          lastSeenAt: new Date(Date.now() - 5_000).toISOString(),
          leaseExpiresAt: new Date(Date.now() + 60_000).toISOString(),
          recoveryMetadata: {
            retryCount: 0,
            history: [],
          },
        });

        const snapshot = await buildSnapshot(deps, missionId);
        const reviewRuntime = snapshot.runtimeProcesses.find((process) => process.featureId === "f2");

        expect(snapshot.session?.agent).toBe("codex");
        expect(snapshot.session?.sessionId).toBe("session-review-1234567890");
        expect(reviewRuntime).toMatchObject({
          featureId: "f2",
          milestoneTitle: "Code Review",
          profile: "code-review",
          agent: "codex",
          sessionId: "session-review-1234567890",
        });
      }, 15_000);
    });
