import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { isAbsolute, join } from "node:path";
import { tmpdir } from "node:os";
import {
  exportBundle,
  type ArchivePort,
  type BundleFile,
  type BundleManifest,
} from "@/features/bundle/index.js";
import { VERSION } from "@/shared/version.js";
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
import type { HandoffRecord, HandoffStorePort } from "@/features/handoff/index.js";
import type { ReplyStorePort, AgentReply } from "@/features/mission/index.js";

const MISSION_ID = "2026-04-15-001";

let projectDir: string;

beforeEach(async () => {
  projectDir = await mkdtemp(join(tmpdir(), "maestro-bundle-export-"));
});

afterEach(async () => {
  await rm(projectDir, { recursive: true, force: true });
});

const mission: Mission = {
  id: MISSION_ID,
  status: "approved",
  title: "Sample",
  description: "desc",
  milestones: [{ id: "m1", title: "m1", description: "", order: 0, featureIds: [] }],
  features: [],
  createdAt: "2026-04-15T00:00:00.000Z",
  updatedAt: "2026-04-15T00:00:00.000Z",
};

class InMemoryArchive implements ArchivePort {
  readonly writes: Array<{ path: string; files: readonly BundleFile[] }> = [];
  async writeTarGz(path: string, files: readonly BundleFile[]) {
    this.writes.push({ path, files });
    return files.reduce((sum, f) => sum + (typeof f.content === "string" ? Buffer.byteLength(f.content) : f.content.length), 0);
  }
  async readManifest(): Promise<BundleManifest> {
    throw new Error("not used");
  }
}

const missionStore: MissionStorePort = {
  async get(id) { return id === MISSION_ID ? mission : undefined; },
  async exists(id) { return id === MISSION_ID; },
  async stage(): Promise<string> { throw new Error("nope"); },
  async finalize() { throw new Error("nope"); },
  async update() { return undefined; },
  async list() { return [mission]; },
  async listIds() { return [MISSION_ID]; },
};

const featureStore: FeatureStorePort = {
  async get() { return undefined; },
  async exists() { return false; },
  async create(): Promise<Feature> { throw new Error("nope"); },
  async update() { return undefined; },
  async list() { return []; },
  async getMany() { return []; },
};

const assertionStore: AssertionStorePort = {
  async get() { return undefined; },
  async exists() { return false; },
  async create(): Promise<Assertion> { throw new Error("nope"); },
  async update() { return undefined; },
  async list() { return []; },
  async listByMilestone() { return []; },
  async getMany() { return []; },
};

const checkpointStore: CheckpointStorePort = {
  async get() { return undefined; },
  async save(): Promise<Checkpoint> { throw new Error("nope"); },
  async list() { return []; },
  async getLatest() { return undefined; },
  async load() { return undefined; },
};

const replyStore: ReplyStorePort = {
  async get() { return undefined; },
  async list(): Promise<readonly AgentReply[]> { return []; },
  async listSince() { return []; },
  async write() { throw new Error("nope"); },
  async isIngested() { return false; },
  async markIngested() { /* noop */ },
};

const handoffStore: HandoffStorePort = {
  async create(): Promise<HandoffRecord> { throw new Error("nope"); },
  async update(record) { return record; },
  async consume(): Promise<HandoffRecord> { throw new Error("nope"); },
  async get() { return undefined; },
  async list() { return []; },
  async listOpenForTask() { return []; },
  resolveArtifactPath(relativePath: string) { return join(projectDir, relativePath); },
};

describe("exportBundle", () => {
  it("writes a manifest with schema v1 and the current maestro version", async () => {
    const archive = new InMemoryArchive();
    const result = await exportBundle(
      { missionStore, featureStore, assertionStore, checkpointStore, replyStore, handoffStore, archive },
      {
        missionId: MISSION_ID,
        projectDir,
        options: { redact: [] },
      },
    );

    expect(result.manifest.schemaVersion).toBe(1);
    expect(result.manifest.maestroVersion).toBe(VERSION);
    expect(result.manifest.mission.id).toBe(MISSION_ID);
    expect(result.manifest.gitPatch).toBeNull();
    expect(archive.writes).toHaveLength(1);

    const manifestFile = archive.writes[0]!.files.find(
      (f) => f.path === `${MISSION_ID}.mission/manifest.json`,
    );
    expect(manifestFile).toBeDefined();
  });

  it("resolves --out to an absolute path", async () => {
    const archive = new InMemoryArchive();
    const result = await exportBundle(
      { missionStore, featureStore, assertionStore, checkpointStore, replyStore, handoffStore, archive },
      {
        missionId: MISSION_ID,
        projectDir,
        options: { redact: [], out: join(projectDir, "rel.tar.gz") },
      },
    );

    expect(isAbsolute(result.outputPath)).toBe(true);
    expect(result.outputPath.endsWith("rel.tar.gz")).toBe(true);
  });

  it("falls back to a timestamped default output name", async () => {
    const archive = new InMemoryArchive();
    const result = await exportBundle(
      { missionStore, featureStore, assertionStore, checkpointStore, replyStore, handoffStore, archive },
      {
        missionId: MISSION_ID,
        projectDir,
        options: { redact: [] },
      },
    );
    expect(result.outputPath).toMatch(new RegExp(`${MISSION_ID}-\\d{8}-\\d{6}\\.mission\\.tar\\.gz$`));
  });
});
