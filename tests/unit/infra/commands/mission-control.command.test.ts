import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsAssertionStoreAdapter } from "@/features/mission";
import { FsCheckpointStoreAdapter } from "@/features/mission";
import { FsFeatureStoreAdapter } from "@/features/mission";
import { FsMissionStoreAdapter } from "@/features/mission";
  import {
    createMissionControlSnapshotLoader,
    loadMissionControlSnapshot,
  } from "@/infra/commands/mission-control.command.js";
  import type { TaskStorePort } from "@/features/task";
import type { ConfigPort } from "@/infra/ports/config.port.js";
import type { GitPort } from "@/infra/ports/git.port.js";
import type { SnapshotDeps } from "@/tui/state/snapshot.js";
import { runCli } from "../../../helpers/run-cli.js";
import { initGitRepo } from "../../../helpers/run-compiled-cli.js";

let tmpDir: string;
let snapshotDeps: SnapshotDeps;

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
    config,
    git,
    cwd: tmpDir,
  };
  });

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function createMission(): Promise<string> {
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(createSamplePlan()));
    const { stdout } = await runCli(["mission", "create", "--file", planPath, "--json"], tmpDir);
    const missionId = JSON.parse(stdout).mission.id as string;
    await runCli(["mission", "approve", missionId, "--json"], tmpDir);
    await runCli(["feature", "update", "f1", "--mission", missionId, "--status", "assigned", "--json"], tmpDir);
    return missionId;
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
      const taskStore: TaskStorePort = {
        create: async () => { throw new Error("unused"); },
        update: async () => { throw new Error("unused"); },
        close: async () => { throw new Error("unused"); },
        get: async () => undefined,
        all: async () => [
          {
            id: "tsk-1",
            title: "Task 1",
            description: "desc",
            type: "task",
            priority: 1,
            status: "open",
            labels: [],
            dependsOn: [],
            createdAt: "2026-01-01T00:00:00Z",
            updatedAt: "2026-01-01T00:00:00Z",
          },
        ],
      };
      const depsWithTasks = { ...snapshotDeps, taskStore };
      const loader = createMissionControlSnapshotLoader(depsWithTasks);

      const homeSnapshot = await loader.load();
      expect(homeSnapshot.taskBoard).toBeUndefined();

      const taskSnapshot = await loader.load({ includeTaskBoard: true });
      expect(taskSnapshot.taskBoard?.totalCount).toBe(1);
    });
  });
