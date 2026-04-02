import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsAssertionStoreAdapter } from "../../../src/adapters/assertion-store.adapter.js";
import { FsCheckpointStoreAdapter } from "../../../src/adapters/checkpoint-store.adapter.js";
import { FsFeatureStoreAdapter } from "../../../src/adapters/feature-store.adapter.js";
import { FsMissionStoreAdapter } from "../../../src/adapters/mission-store.adapter.js";
import { FsRuntimeStoreAdapter } from "../../../src/adapters/runtime-store.adapter.js";
import { FsRuntimeEventStoreAdapter } from "../../../src/adapters/runtime-event-store.adapter.js";
import {
  createMissionControlSnapshotLoader,
  loadMissionControlSnapshot,
  type MissionControlSnapshotLoadMode,
} from "../../../src/commands/mission-control.command.js";
import type { CassPort } from "../../../src/ports/cass.port.js";
import type { ConfigPort } from "../../../src/ports/config.port.js";
import type { GitPort } from "../../../src/ports/git.port.js";
import type { HandoffStorePort } from "../../../src/ports/handoff-store.port.js";
import type { SnapshotDeps, HomeSnapshotDeps } from "../../../src/tui/state/snapshot.js";

let tmpDir: string;
let snapshotDeps: SnapshotDeps;
let homeSnapshotDeps: HomeSnapshotDeps;
let runtimeStore: FsRuntimeStoreAdapter;
let runtimeEventStore: FsRuntimeEventStoreAdapter;

const CLI = ["bun", "run", join(import.meta.dir, "..", "..", "..", "src", "index.ts")];

async function initGitRepo(cwd: string): Promise<void> {
  const proc = Bun.spawn(["git", "init", "-b", "main"], { cwd, stdout: "pipe", stderr: "pipe" });
  await proc.exited;
}

async function run(args: string[], cwd: string): Promise<{ stdout: string; exitCode: number }> {
  const proc = Bun.spawn([...CLI, ...args], { stdout: "pipe", stderr: "pipe", cwd });
  const stdout = await new Response(proc.stdout).text();
  const exitCode = await proc.exited;
  return { stdout: stdout.trim(), exitCode };
}

function createSamplePlan(): object {
  return {
    title: "Mission Control Command Test Mission",
    description: "Testing mission-control command loading",
    milestones: [
      { id: "m1", title: "Milestone 1", description: "First", order: 0 },
    ],
    features: [
      { id: "f1", milestoneId: "m1", title: "Feature 1", description: "First feature", workerType: "test-skill", verificationSteps: ["check it"] },
    ],
  };
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-mission-control-command-"));
  await initGitRepo(tmpDir);
  runtimeStore = new FsRuntimeStoreAdapter(tmpDir);
  runtimeEventStore = new FsRuntimeEventStoreAdapter(tmpDir);

  const handoffStore = {
    create: async () => "handoff-id",
    get: async () => undefined,
    getLatestPending: async () => undefined,
    listIds: async () => [],
    list: async () => [],
    updateStatus: async () => undefined,
    delete: async () => undefined,
  } satisfies HandoffStorePort;
    const config = {
      load: async () => ({ defaultAgent: "codex" }),
      loadLayers: async () => ({
        defaults: { defaultAgent: "codex" },
        effective: { defaultAgent: "codex" },
        project: { defaultAgent: "codex" },
        global: undefined,
        errors: [],
        paths: {
          project: ".maestro/config.yaml",
          global: "~/.maestro/config.yaml",
        },
      }),
      write: async () => undefined,
      exists: async () => true,
    } satisfies ConfigPort;
  const cass = {
    isAvailable: async () => true,
    hasBinary: async () => true,
    indexOnce: async () => undefined,
    search: async () => ({ query: "", hits: [], count: 0, totalMatches: 0 }),
  } satisfies CassPort;
  const git = {
    getState: async () => ({
      branch: "main",
      recentCommits: [],
      changedFiles: ["src/index.ts"],
      workingTreeClean: false,
      diffStat: "+1 -0",
    }),
    isRepo: async () => true,
  } satisfies GitPort;

  snapshotDeps = {
    missionStore: new FsMissionStoreAdapter(tmpDir),
    featureStore: new FsFeatureStoreAdapter(tmpDir),
    assertionStore: new FsAssertionStoreAdapter(tmpDir),
    checkpointStore: new FsCheckpointStoreAdapter(tmpDir),
    handoffStore,
    config,
    cass,
    git,
    runtimeStore,
    runtimeEventStore,
    cwd: tmpDir,
  };
  homeSnapshotDeps = { handoffStore, config, cass, git };
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function createMissionAndRuntime(mode: MissionControlSnapshotLoadMode): Promise<string> {
  const planPath = join(tmpDir, "plan.json");
  await writeFile(planPath, JSON.stringify(createSamplePlan()));
  const { stdout } = await run(["mission", "create", "--file", planPath, "--json"], tmpDir);
  const missionId = JSON.parse(stdout).mission.id as string;
  await run(["mission", "approve", missionId, "--json"], tmpDir);
  await run(["feature", "update", "f1", "--mission", missionId, "--status", "assigned", "--json"], tmpDir);

  await runtimeStore.save(missionId, "f1", {
    featureId: "f1",
    attemptId: `attempt-${mode}`,
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

  return missionId;
}

describe("loadMissionControlSnapshot", () => {
  it("keeps read mode inspection non-mutating", async () => {
    const missionId = await createMissionAndRuntime("read");

    const snapshot = await loadMissionControlSnapshot(snapshotDeps, homeSnapshotDeps, "read", missionId);

    expect(snapshot.activeWorker).toMatchObject({
      featureId: "f1",
      runtimeState: "failed",
    });
    expect(await snapshotDeps.featureStore.get(missionId, "f1")).toMatchObject({ status: "assigned" });
    expect(await runtimeStore.get(missionId, "f1")).toMatchObject({
      runtimeState: "live",
      recoveryMetadata: { retryCount: 0, history: [] },
    });
  });

  it("applies runtime recovery in supervise mode before projecting the snapshot", async () => {
    const missionId = await createMissionAndRuntime("supervise");

    const snapshot = await loadMissionControlSnapshot(snapshotDeps, homeSnapshotDeps, "supervise", missionId);

    expect(snapshot.activeFeature).toMatchObject({
      id: "f1",
      status: "pending",
      runtimeState: "recoverable",
      retryCount: 1,
    });
    expect(await snapshotDeps.featureStore.get(missionId, "f1")).toMatchObject({ status: "pending" });
    expect(await runtimeStore.get(missionId, "f1")).toMatchObject({
      runtimeState: "recoverable",
      recoveryMetadata: { retryCount: 1 },
    });
  });

  it("re-resolves the mission after starting in home mode without an explicit mission", async () => {
    const loader = createMissionControlSnapshotLoader(
      snapshotDeps,
      homeSnapshotDeps,
      "read",
    );

    const homeSnapshot = await loader.load();
    expect(homeSnapshot.mode).toBe("home");

    const missionId = await createMissionAndRuntime("read");
    const missionSnapshot = await loader.load();

    expect(missionSnapshot.mode).toBe("mission");
    expect(missionSnapshot.missionId).toBe(missionId);
  });
});
