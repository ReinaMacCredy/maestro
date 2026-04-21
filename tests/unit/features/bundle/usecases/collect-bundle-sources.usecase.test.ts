import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { collectBundleSources } from "@/features/bundle/usecases/collect-bundle-sources.usecase.js";
import type { BundleFile } from "@/features/bundle/domain/bundle-types.js";
import { MaestroError } from "@/shared/errors.js";
import type {
  Assertion,
  AssertionStorePort,
  Checkpoint,
  CheckpointStorePort,
  Feature,
  FeatureStorePort,
  Mission,
  MissionStorePort,
} from "@/features/mission/index.js";
import type { HandoffLaunchRecord, LaunchStorePort } from "@/features/handoff/index.js";
import type { ReplyStorePort } from "@/features/reply/index.js";
import type { AgentReply } from "@/features/reply/index.js";

const MISSION_ID = "2026-04-13-001";

let projectDir: string;

beforeEach(async () => {
  projectDir = await mkdtemp(join(tmpdir(), "maestro-bundle-collect-"));
});

afterEach(async () => {
  await rm(projectDir, { recursive: true, force: true });
});

function buildMission(): Mission {
  return {
    id: MISSION_ID,
    status: "approved",
    title: "Bundle test",
    description: "desc",
    milestones: [
      { id: "m1", title: "M1", description: "one", order: 0, featureIds: ["f1"] },
      { id: "m2", title: "M2", description: "two", order: 1, featureIds: ["f2"] },
    ],
    features: ["f1", "f2"],
    createdAt: "2026-04-13T00:00:00.000Z",
    updatedAt: "2026-04-13T00:00:00.000Z",
  };
}

function buildFeature(id: string, milestoneId: string): Feature {
  return {
    id,
    missionId: MISSION_ID,
    milestoneId,
    status: "done",
    title: `Feature ${id}`,
    description: "desc",
    agentType: "implementer",
    verificationSteps: [],
    dependsOn: [],
    fulfills: [],
    createdAt: "2026-04-13T00:00:00.000Z",
    updatedAt: "2026-04-13T00:00:00.000Z",
  };
}

function buildAssertion(id: string, milestoneId: string): Assertion {
  return {
    id,
    missionId: MISSION_ID,
    milestoneId,
    featureId: "f1",
    result: "passed",
    description: `Check ${id}`,
    surface: "cli",
    createdAt: "2026-04-13T00:00:00.000Z",
    updatedAt: "2026-04-13T00:00:00.000Z",
  };
}

function buildCheckpoint(id: string): Checkpoint {
  return {
    id,
    missionId: MISSION_ID,
    currentMilestoneId: "m1",
    timestamp: "2026-04-13T01:00:00.000Z",
    featureStatuses: { f1: "done" },
    assertionResults: { a1: "passed" },
  };
}

function buildLaunch(id: string, refs: HandoffLaunchRecord["refs"]): HandoffLaunchRecord {
  return {
    id,
    createdAt: "2026-04-13T02:00:00.000Z",
    task: "bundle launch",
    name: "[Handoff] bundle launch",
    agent: "claude",
    model: "opus",
    status: "completed",
    wait: true,
    sourceDir: projectDir,
    targetDir: projectDir,
    promptPath: `.maestro/launches/${id}/prompt.md`,
    outputPath: `.maestro/launches/${id}/output.log`,
    command: ["claude", "--print", "bundle launch"],
    refs,
  };
}

function buildReply(featureId: string): AgentReply {
  return {
    missionId: MISSION_ID,
    featureId,
    outcome: "completed",
    writtenAt: "2026-04-13T03:00:00.000Z",
    writtenBy: "human",
  };
}

class FakeMissionStore implements MissionStorePort {
  constructor(private readonly missions: ReadonlyMap<string, Mission>) {}
  async get(id: string) { return this.missions.get(id); }
  async exists(id: string) { return this.missions.has(id); }
  async stage(): Promise<string> { throw new Error("not implemented"); }
  async finalize() { throw new Error("not implemented"); }
  async update() { return undefined; }
  async list() { return [...this.missions.values()]; }
  async listIds() { return [...this.missions.keys()]; }
}

class FakeFeatureStore implements FeatureStorePort {
  constructor(private readonly features: readonly Feature[]) {}
  async get(_missionId: string, id: string) {
    return this.features.find((f) => f.id === id);
  }
  async exists(_m: string, id: string) { return this.features.some((f) => f.id === id); }
  async create(): Promise<Feature> { throw new Error("not implemented"); }
  async update() { return undefined; }
  async list() { return this.features; }
  async getMany(_m: string, ids: readonly string[]) {
    return this.features.filter((f) => ids.includes(f.id));
  }
}

class FakeAssertionStore implements AssertionStorePort {
  constructor(private readonly assertions: readonly Assertion[]) {}
  async get(_m: string, id: string) {
    return this.assertions.find((a) => a.id === id);
  }
  async exists(_m: string, id: string) { return this.assertions.some((a) => a.id === id); }
  async create(): Promise<Assertion> { throw new Error("not implemented"); }
  async update() { return undefined; }
  async list() { return this.assertions; }
  async listByMilestone(_m: string, milestoneId: string) {
    return this.assertions.filter((a) => a.milestoneId === milestoneId);
  }
  async getMany(_m: string, ids: readonly string[]) {
    return this.assertions.filter((a) => ids.includes(a.id));
  }
}

class FakeCheckpointStore implements CheckpointStorePort {
  constructor(private readonly checkpoints: readonly Checkpoint[]) {}
  async get(_m: string, id: string) { return this.checkpoints.find((c) => c.id === id); }
  async save(): Promise<Checkpoint> { throw new Error("not implemented"); }
  async list() { return this.checkpoints; }
  async getLatest() { return this.checkpoints[0]; }
  async load() { return this.checkpoints[0]; }
}

class FakeReplyStore implements ReplyStorePort {
  constructor(private readonly replies: ReadonlyMap<string, AgentReply>) {}
  async get(missionId: string, featureId: string) {
    return this.replies.get(`${missionId}:${featureId}`);
  }
  async list() { return [...this.replies.values()]; }
  async listSince() { return [...this.replies.values()]; }
  async write() { throw new Error("not implemented"); }
  async isIngested() { return false; }
  async markIngested() { /* noop */ }
}

class FakeLaunchStore implements LaunchStorePort {
  constructor(private readonly launches: readonly HandoffLaunchRecord[]) {}
  async create(): Promise<HandoffLaunchRecord> { throw new Error("not implemented"); }
  async update(record: HandoffLaunchRecord) { return record; }
  async consume(): Promise<HandoffLaunchRecord> { throw new Error("not implemented"); }
  async get(id: string) { return this.launches.find((launch) => launch.id === id); }
  async list() { return this.launches; }
  resolveArtifactPath(relativePath: string) { return join(projectDir, relativePath); }
}

async function seedAgents(featureId: string): Promise<void> {
  const dir = join(projectDir, ".maestro", "missions", MISSION_ID, "agents", featureId);
  await mkdir(dir, { recursive: true });
  await writeFile(join(dir, "prompt.md"), `# prompt for ${featureId}\n`);
  await writeFile(
    join(dir, "report.json"),
    JSON.stringify({ feature: featureId }),
  );
}

async function seedReplies(featureId: string): Promise<void> {
  const dir = join(projectDir, ".maestro", "replies", MISSION_ID);
  await mkdir(dir, { recursive: true });
  await writeFile(
    join(dir, `${featureId}.yaml`),
    `missionId: ${MISSION_ID}\nfeatureId: ${featureId}\noutcome: completed\n`,
  );
}

async function seedPrinciplesAndOutcomes(): Promise<void> {
  await mkdir(join(projectDir, ".maestro"), { recursive: true });
  await writeFile(
    join(projectDir, ".maestro", "principles.jsonl"),
    `${JSON.stringify({ id: "p1", rule: "Prefer simplicity" })}\n${JSON.stringify({ id: "p2", rule: "Cover edges" })}\n`,
  );
  await mkdir(join(projectDir, ".maestro", "principles"), { recursive: true });
  await writeFile(
    join(projectDir, ".maestro", "principles", "outcomes.jsonl"),
    [
      JSON.stringify({ principleId: "p1", handoffId: "h1", missionId: MISSION_ID, outcome: "helpful", recordedAt: "2026-04-13T00:00:00.000Z" }),
      JSON.stringify({ principleId: "p1", handoffId: "h2", missionId: "2020-01-01-999", outcome: "helpful", recordedAt: "2026-04-13T00:00:00.000Z" }),
    ].join("\n") + "\n",
  );
}

async function seedMemory(): Promise<void> {
  const correctionsDir = join(projectDir, ".maestro", "memory", "corrections");
  await mkdir(correctionsDir, { recursive: true });
  await writeFile(join(correctionsDir, "c1.json"), JSON.stringify({ id: "c1" }));

  const learningsDir = join(projectDir, ".maestro", "memory", "learnings");
  await mkdir(learningsDir, { recursive: true });
  await writeFile(
    join(learningsDir, "_compiled.json"),
    JSON.stringify({ learnings: [{ id: "l1" }, { id: "l2" }] }),
  );
}

function makeDeps({
  replies = new Map<string, AgentReply>(),
  launches = [] as readonly HandoffLaunchRecord[],
  checkpoints = [] as readonly Checkpoint[],
  assertions = [] as readonly Assertion[],
} = {}) {
  const mission = buildMission();
  return {
    missionStore: new FakeMissionStore(new Map([[mission.id, mission]])),
    featureStore: new FakeFeatureStore([
      buildFeature("f1", "m1"),
      buildFeature("f2", "m2"),
    ]),
    assertionStore: new FakeAssertionStore(assertions),
    checkpointStore: new FakeCheckpointStore(checkpoints),
    replyStore: new FakeReplyStore(replies),
    launchStore: new FakeLaunchStore(launches),
  };
}

function filesByPath(files: readonly BundleFile[]): Map<string, BundleFile> {
  return new Map(files.map((f) => [f.path, f]));
}

describe("collectBundleSources", () => {
  it("aggregates a minimal mission without replies, launches, or memory", async () => {
    const deps = makeDeps();
    const result = await collectBundleSources(deps, {
      missionId: MISSION_ID,
      projectDir,
      options: { redact: [] },
    });

    const files = filesByPath(result.files);
    expect(files.has(`${MISSION_ID}.mission/mission/mission.json`)).toBe(true);
    expect(files.has(`${MISSION_ID}.mission/mission/features/f1.json`)).toBe(true);
    expect(files.has(`${MISSION_ID}.mission/mission/features/f2.json`)).toBe(true);
    expect(files.has(`${MISSION_ID}.mission/mission/assertions.json`)).toBe(true);
    expect(result.stats.features).toBe(2);
    expect(result.stats.milestones).toBe(2);
    expect(result.stats.agents).toBe(0);
    expect(result.stats.replies).toBe(0);
    expect(result.stats.launches).toBe(0);
    expect(result.stats.memorySnapshot).toEqual({ corrections: 0, learnings: 0 });
  });

  it("includes agent files, replies, launches, checkpoints, principles, and memory", async () => {
    await seedAgents("f1");
    await seedAgents("f2");
    await seedReplies("f1");
    await seedPrinciplesAndOutcomes();
    await seedMemory();

    const launchInScope = buildLaunch("2026-04-13-101", { missionId: MISSION_ID });
    const launchOutOfScope = buildLaunch("2026-04-13-102", { missionId: "2020-01-01-001" });

    const replies = new Map<string, AgentReply>([
      [`${MISSION_ID}:f1`, buildReply("f1")],
    ]);

    const deps = makeDeps({
      replies,
      launches: [launchInScope, launchOutOfScope],
      checkpoints: [buildCheckpoint("20260413-010000-000")],
      assertions: [buildAssertion("a1", "m1")],
    });

    const result = await collectBundleSources(deps, {
      missionId: MISSION_ID,
      projectDir,
      options: { redact: [] },
    });

    const files = filesByPath(result.files);
    expect(files.has(`${MISSION_ID}.mission/mission/agents/f1/prompt.md`)).toBe(true);
    expect(files.has(`${MISSION_ID}.mission/mission/agents/f1/report.json`)).toBe(true);
    expect(files.has(`${MISSION_ID}.mission/mission/agents/f2/prompt.md`)).toBe(true);
    expect(files.has(`${MISSION_ID}.mission/replies/f1.yaml`)).toBe(true);
    expect(files.has(`${MISSION_ID}.mission/launches/2026-04-13-101.json`)).toBe(true);
    expect(files.has(`${MISSION_ID}.mission/launches/2026-04-13-102.json`)).toBe(false);
    expect(files.has(`${MISSION_ID}.mission/mission/checkpoints/20260413-010000-000.json`)).toBe(true);
    expect(files.has(`${MISSION_ID}.mission/principles/principles.jsonl`)).toBe(true);
    expect(files.has(`${MISSION_ID}.mission/principles/outcomes.jsonl`)).toBe(true);
    expect(files.has(`${MISSION_ID}.mission/memory/corrections/c1.json`)).toBe(true);
    expect(files.has(`${MISSION_ID}.mission/memory/learnings/_compiled.json`)).toBe(true);

    expect(result.stats.features).toBe(2);
    expect(result.stats.agents).toBe(2);
    expect(result.stats.replies).toBe(1);
    expect(result.stats.launches).toBe(1);
    expect(result.stats.checkpoints).toBe(1);
    expect(result.stats.assertions).toBe(1);
    expect(result.stats.principlesSnapshot).toBe(2);
    expect(result.stats.outcomesSnapshot).toBe(1); // unrelated mission filtered out
    expect(result.stats.memorySnapshot).toEqual({ corrections: 1, learnings: 2 });
  });

  it("drops redacted scopes from the output", async () => {
    await seedAgents("f1");
    await seedReplies("f1");
    await seedMemory();

    const replies = new Map<string, AgentReply>([
      [`${MISSION_ID}:f1`, buildReply("f1")],
    ]);
    const deps = makeDeps({ replies });

    const result = await collectBundleSources(deps, {
      missionId: MISSION_ID,
      projectDir,
      options: { redact: ["memory", "prompts", "replies"] },
    });

    const files = filesByPath(result.files);
    expect(files.has(`${MISSION_ID}.mission/mission/agents/f1/prompt.md`)).toBe(false);
    expect(files.has(`${MISSION_ID}.mission/mission/agents/f1/report.json`)).toBe(true);
    expect(files.has(`${MISSION_ID}.mission/replies/f1.yaml`)).toBe(false);
    expect([...files.keys()].some((p) => p.startsWith(`${MISSION_ID}.mission/memory/`))).toBe(false);

    expect(result.stats.replies).toBe(0);
    expect(result.stats.memorySnapshot).toBeNull();
  });

  it("throws when the mission does not exist", async () => {
    const deps = makeDeps();
    const promise = collectBundleSources(deps, {
      missionId: "2020-01-01-999",
      projectDir,
      options: { redact: [] },
    });

    await expect(promise).rejects.toBeInstanceOf(MaestroError);
    await expect(promise).rejects.toThrow(/not found/);
  });

  it("rejects unsafe mission ids", async () => {
    const deps = makeDeps();
    await expect(
      collectBundleSources(deps, {
        missionId: "../escape",
        projectDir,
        options: { redact: [] },
      }),
    ).rejects.toThrow(/mission ID/);
  });
});
