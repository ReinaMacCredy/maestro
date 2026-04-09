import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsAssertionStoreAdapter } from "@/adapters/assertion-store.adapter.js";
import { FsCheckpointStoreAdapter } from "@/adapters/checkpoint-store.adapter.js";
import { FsFeatureStoreAdapter } from "@/adapters/feature-store.adapter.js";
import { FsMissionStoreAdapter } from "@/adapters/mission-store.adapter.js";
import {
  createMissionControlSnapshotLoader,
  loadMissionControlSnapshot,
} from "@/commands/mission-control.command.js";
import type { ConfigPort } from "@/ports/config.port.js";
import type { GitPort } from "@/ports/git.port.js";
import type { SnapshotDeps, HomeSnapshotDeps } from "@/tui/state/snapshot.js";

let tmpDir: string;
let snapshotDeps: SnapshotDeps;
let homeSnapshotDeps: HomeSnapshotDeps;

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
  homeSnapshotDeps = { config, git };
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function createMission(): Promise<string> {
  const planPath = join(tmpDir, "plan.json");
  await writeFile(planPath, JSON.stringify(createSamplePlan()));
  const { stdout } = await run(["mission", "create", "--file", planPath, "--json"], tmpDir);
  const missionId = JSON.parse(stdout).mission.id as string;
  await run(["mission", "approve", missionId, "--json"], tmpDir);
  await run(["feature", "update", "f1", "--mission", missionId, "--status", "assigned", "--json"], tmpDir);
  return missionId;
}

describe("loadMissionControlSnapshot", () => {
  it("keeps read mode inspection non-mutating", async () => {
    const missionId = await createMission();

    const snapshot = await loadMissionControlSnapshot(snapshotDeps, homeSnapshotDeps, "read", missionId);

    expect(snapshot.mode).toBe("mission");
    expect(snapshot.missionId).toBe(missionId);
    expect(await snapshotDeps.featureStore.get(missionId, "f1")).toMatchObject({ status: "assigned" });
  });

  it("re-resolves the mission after starting in home mode without an explicit mission", async () => {
    const loader = createMissionControlSnapshotLoader(
      snapshotDeps,
      homeSnapshotDeps,
      "read",
    );

    const homeSnapshot = await loader.load();
    expect(homeSnapshot.mode).toBe("home");

    const missionId = await createMission();
    const missionSnapshot = await loader.load();

    expect(missionSnapshot.mode).toBe("mission");
    expect(missionSnapshot.missionId).toBe(missionId);
  });
});
