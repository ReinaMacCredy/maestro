import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsAssertionStoreAdapter } from "@/shared/domain/legacy-mission";
import { FsCheckpointStoreAdapter } from "@/shared/domain/legacy-mission";
import { FsFeatureStoreAdapter } from "@/shared/domain/legacy-mission";
import { FsMissionStoreAdapter } from "@/shared/domain/legacy-mission";
import { buildMissions, createMission as createMissionRecord, updateFeature } from "@/shared/domain/legacy-mission";
  import {
    createMissionControlSnapshotLoader,
    loadMissionControlSnapshot,
  } from "@/infra/commands/mission-control.command.js";
  import type { LegacyTask as Task, TaskQueryPort } from "@/shared/domain/legacy-task";
import type { ConfigPort } from "@/infra/ports/config.port.js";
import type { GitPort } from "@/infra/ports/git.port.js";
import type { SnapshotDeps } from "@/tui/state/snapshot.js";
import { runCli } from "../../../helpers/run-cli.js";
import { initGitRepo } from "../../../helpers/run-compiled-cli.js";

let tmpDir: string;
let snapshotDeps: SnapshotDeps;

function makeTaskQueryStore(tasks: readonly Task[]): TaskQueryPort {
  return {
    get: async (id: string) => tasks.find((task) => task.id === id),
    all: async () => tasks,
  };
}

function createSamplePlan(): object {
  return {
    title: "Mission Control Command Test Mission",
    description: "Testing mission-control command loading",
    milestones: [
      { id: "m1", title: "Milestone 1", description: "First", order: 0 },
    ],
    features: [
      { id: "f1", milestoneId: "m1", title: "Feature 1", description: "First feature", agentType: "test-skill", verificationSteps: ["check it"] },
    ],
  };
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-mission-control-command-"));
  await initGitRepo(tmpDir);

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
  const git = {
    getState: async () => ({
      branch: "main",
      recentCommits: [],
      changedFiles: ["src/index.ts"],
      fileChanges: [],
      workingTreeClean: false,
      diffStat: "+1 -0",
    }),
    isRepo: async () => true,
    getCurrentBranch: async () => "main",
    createWorktree: async () => ({
      slug: "test",
      baseBranch: "main",
      branch: "feat/test",
      path: "/tmp/test",
    }),
  } satisfies GitPort;

  const missionStore = new FsMissionStoreAdapter(tmpDir);
  const featureStore = new FsFeatureStoreAdapter(tmpDir);
  const assertionStore = new FsAssertionStoreAdapter(tmpDir);
  const checkpointStore = new FsCheckpointStoreAdapter(tmpDir);
  snapshotDeps = {
    missions: buildMissions(missionStore, featureStore, assertionStore, checkpointStore),
    missionStore,
    featureStore,
    assertionStore,
    checkpointStore,
    config,
    git,
    cwd: tmpDir,
  };
  });

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function createMission(): Promise<string> {
  const missionStore = new FsMissionStoreAdapter(tmpDir);
  const featureStore = new FsFeatureStoreAdapter(tmpDir);
  const assertionStore = new FsAssertionStoreAdapter(tmpDir);
  const plan = createSamplePlan() as Parameters<typeof createMissionRecord>[3];
  const { mission } = await createMissionRecord(missionStore, featureStore, assertionStore, plan);
  await missionStore.update(mission.id, { status: "approved" });
  await updateFeature(missionStore, featureStore, tmpDir, mission.id, "f1", { status: "assigned" });
  return mission.id;
}

describe("loadMissionControlSnapshot", () => {
  it("keeps inspection non-mutating", async () => {
    const missionId = await createMission();

      const snapshot = await loadMissionControlSnapshot(snapshotDeps, missionId);

    expect(snapshot.mode).toBe("mission");
    expect(snapshot.missionId).toBe(missionId);
    expect(await snapshotDeps.featureStore.get(missionId, "f1")).toMatchObject({ status: "assigned" });
  });

  it("re-resolves the mission after starting in home mode without an explicit mission", async () => {
      const loader = createMissionControlSnapshotLoader(
        snapshotDeps,
      );

    const homeSnapshot = await loader.load();
    expect(homeSnapshot.mode).toBe("home");

    const missionId = await createMission();
    const missionSnapshot = await loader.load();

      expect(missionSnapshot.mode).toBe("mission");
      expect(missionSnapshot.missionId).toBe(missionId);
    });

  it("loads task-board data only when requested", async () => {
      const taskStore = makeTaskQueryStore([
        {
          id: "tsk-1",
          title: "Task 1",
          description: "desc",
          type: "task",
          priority: 1,
          status: "pending",
          labels: [],
          blocks: [],
          blockedBy: [],
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-01T00:00:00Z",
        },
      ]);
      const depsWithTasks = { ...snapshotDeps, taskStore };
      const loader = createMissionControlSnapshotLoader(depsWithTasks);

      const homeSnapshot = await loader.load();
      expect(homeSnapshot.taskBoard).toBeUndefined();

    const taskSnapshot = await loader.load({ includeTaskBoard: true });
    expect(taskSnapshot.taskBoard?.totalCount).toBe(1);
  });

  it("loads task-board data for mission snapshots when requested", async () => {
    const missionId = await createMission();
      const taskStore = makeTaskQueryStore([
        {
          id: "tsk-1",
          title: "Task 1",
          description: "desc",
          type: "task",
          priority: 1,
          status: "pending",
          labels: [],
          blocks: [],
          blockedBy: [],
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-01T00:00:00Z",
        },
      ]);
    const loader = createMissionControlSnapshotLoader(
      { ...snapshotDeps, taskStore },
      missionId,
    );

    const missionSnapshot = await loader.load();
    expect(missionSnapshot.taskBoard).toBeUndefined();

    const taskSnapshot = await loader.load({ includeTaskBoard: true });
    expect(taskSnapshot.taskBoard?.totalCount).toBe(1);
  });

  it("does not expose a pendingHandoffs field in json output", async () => {
    const missionId = await createMission();

    const { stdout } = await runCli(
      ["mission-control", "--mission", missionId, "--json"],
      tmpDir,
    );
    const snapshot = JSON.parse(stdout) as Record<string, unknown>;

    expect(snapshot).not.toHaveProperty("pendingHandoffs");
  });
});
