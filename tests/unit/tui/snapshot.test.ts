import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { buildSnapshot, type SnapshotDeps } from "../../../src/tui/snapshot.js";
import { FsMissionStoreAdapter } from "../../../src/adapters/mission-store.adapter.js";
import { FsFeatureStoreAdapter } from "../../../src/adapters/feature-store.adapter.js";
import { FsAssertionStoreAdapter } from "../../../src/adapters/assertion-store.adapter.js";
import { FsCheckpointStoreAdapter } from "../../../src/adapters/checkpoint-store.adapter.js";

let tmpDir: string;
let deps: SnapshotDeps;

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
  deps = {
    missionStore: new FsMissionStoreAdapter(tmpDir),
    featureStore: new FsFeatureStoreAdapter(tmpDir),
    assertionStore: new FsAssertionStoreAdapter(tmpDir),
    checkpointStore: new FsCheckpointStoreAdapter(tmpDir),
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
});
