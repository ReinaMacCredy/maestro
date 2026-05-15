import { afterEach, beforeEach, describe, expect, it, mock } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { Command } from "commander";
import type {
  ArchivePort,
  BundleFile,
  BundleManifest,
} from "@/features/bundle/index.js";
import type {
  Assertion,
  AssertionStorePort,
  Checkpoint,
  CheckpointStorePort,
  Feature,
  FeatureStorePort,
  Mission,
  MissionStorePort,
} from "@/shared/domain/legacy-mission";
import { buildMissions } from "@/shared/domain/legacy-mission";
import type { HandoffRecord, HandoffStorePort } from "@/features/handoff/index.js";
import type { ReplyStorePort, AgentReply } from "@/features/mission/index.js";

const originalConsoleLog = console.log;
const originalConsoleError = console.error;

let tmpDir: string;

function captureConsole(): { readonly logs: string[]; readonly errors: string[] } {
  const logs: string[] = [];
  const errors: string[] = [];
  console.log = (...args: unknown[]) => {
    logs.push(args.map((arg) => String(arg)).join(" "));
  };
  console.error = (...args: unknown[]) => {
    errors.push(args.map((arg) => String(arg)).join(" "));
  };
  return { logs, errors };
}

const mission: Mission = {
  id: "2026-04-15-001",
  status: "draft",
  title: "Test Mission",
  description: "desc",
  milestones: [{ id: "m1", title: "m1", description: "", order: 0, featureIds: [] }],
  features: [],
  createdAt: "2026-04-14T00:00:00.000Z",
  updatedAt: "2026-04-14T00:00:00.000Z",
};

const missionStore: MissionStorePort = {
  async get(id) { return id === mission.id ? mission : undefined; },
  async exists(id) { return id === mission.id; },
  async stage(): Promise<string> { throw new Error("nope"); },
  async finalize() { throw new Error("nope"); },
  async update() { return undefined; },
  async list() { return [mission]; },
  async listIds() { return [mission.id]; },
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
  resolveArtifactPath(relativePath: string) { return join(tmpDir, relativePath); },
};

class RecordingArchive implements ArchivePort {
  readonly writes: Array<{ path: string; files: readonly BundleFile[] }> = [];
  readManifestCalls: string[] = [];
  nextManifest?: BundleManifest;

  async writeTarGz(path: string, files: readonly BundleFile[]) {
    this.writes.push({ path, files });
    return 1024;
  }

  async readManifest(path: string): Promise<BundleManifest> {
    this.readManifestCalls.push(path);
    if (!this.nextManifest) throw new Error("manifest not set");
    return this.nextManifest;
  }
}

let archive: RecordingArchive;

async function loadRegisterBundleCommand() {
  archive = new RecordingArchive();
  const mod = await import("@/features/bundle/commands/bundle.command.js");
  const deps = {
    getServices: () => ({
      missionStore,
      featureStore,
      assertionStore,
      checkpointStore,
      missions: buildMissions(missionStore, featureStore, assertionStore, checkpointStore),
      replyStore,
      handoffStore,
      archive,
      sessionDetect: undefined,
    } as never),
  };
  return {
    registerBundleCommand: (program: Command): void => mod.registerBundleCommand(program, deps),
  };
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-bundle-command-"));
});

afterEach(async () => {
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
  mock.restore();
  await rm(tmpDir, { recursive: true, force: true });
});

describe("bundle commands", () => {
  it("runs bundle export against the real usecase and formats human output", async () => {
    const captured = captureConsole();
    const { registerBundleCommand } = await loadRegisterBundleCommand();

    const program = new Command().name("maestro").option("--json");
    registerBundleCommand(program);

    await program.parseAsync([
      "node",
      "maestro",
      "bundle",
      "export",
      "2026-04-15-001",
      "--out",
      join(tmpDir, "out.tar.gz"),
    ]);

    expect(archive.writes).toHaveLength(1);
    expect(archive.writes[0]!.path.endsWith("out.tar.gz")).toBe(true);
    expect(archive.writes[0]!.files.some((f) => f.path.endsWith("manifest.json"))).toBe(true);

    expect(captured.logs[0]).toBe("[ok] Bundle exported");
    expect(captured.logs.some((l) => l.includes("Mission: 2026-04-15-001"))).toBe(true);
  });

  it("parses comma-separated --redact flags and drops memory files", async () => {
    captureConsole();
    const { registerBundleCommand } = await loadRegisterBundleCommand();

    const program = new Command().name("maestro").option("--json");
    registerBundleCommand(program);

    await program.parseAsync([
      "node",
      "maestro",
      "bundle",
      "export",
      "2026-04-15-001",
      "--out",
      join(tmpDir, "redacted.tar.gz"),
      "--redact",
      "memory,prompts",
    ]);

    expect(archive.writes).toHaveLength(1);
    const files = archive.writes[0]!.files;
    expect(files.some((f) => f.path.includes("/memory/"))).toBe(false);
    const manifestFile = files.find((f) => f.path.endsWith("manifest.json"))!;
    const manifest = JSON.parse(manifestFile.content as string) as BundleManifest;
    expect(manifest.redacted).toEqual(["memory", "prompts"]);
    expect(manifest.stats.memorySnapshot).toBeNull();
  });

  it("rejects unknown --redact scopes", async () => {
    const { registerBundleCommand } = await loadRegisterBundleCommand();

    const program = new Command().name("maestro").option("--json");
    registerBundleCommand(program);

    await expect(
      program.parseAsync([
        "node",
        "maestro",
        "bundle",
        "export",
        "2026-04-15-001",
        "--redact",
        "memory,bogus",
      ]),
    ).rejects.toMatchObject({ message: expect.stringContaining("bogus") });
  });

  it("prints export result as JSON when --json is set", async () => {
    const captured = captureConsole();
    const { registerBundleCommand } = await loadRegisterBundleCommand();

    const program = new Command().name("maestro").option("--json");
    registerBundleCommand(program);

    await program.parseAsync([
      "node",
      "maestro",
      "bundle",
      "export",
      "2026-04-15-001",
      "--out",
      join(tmpDir, "jsonout.tar.gz"),
      "--json",
    ]);

    const parsed = JSON.parse(captured.logs.join("\n"));
    expect(parsed.outputPath.endsWith("jsonout.tar.gz")).toBe(true);
    expect(parsed.manifest.schemaVersion).toBe(1);
    expect(parsed.manifest.mission.id).toBe("2026-04-15-001");
  });

  it("delegates bundle inspect to the archive port and formats human output", async () => {
    const captured = captureConsole();
    const { registerBundleCommand } = await loadRegisterBundleCommand();
    archive.nextManifest = {
      schemaVersion: 1,
      bundleId: "abc-123",
      createdAt: "2026-04-15T00:00:00.000Z",
      maestroVersion: "0.0.0",
      mission: {
        id: "2026-04-15-001",
        title: "Fixture",
        status: "draft",
        createdAt: "2026-04-14T00:00:00.000Z",
      },
      stats: {
        features: 0,
        milestones: 0,
        assertions: 0,
        agents: 0,
        replies: 0,
        handoffs: 0,
        checkpoints: 0,
        principlesSnapshot: 0,
        outcomesSnapshot: 0,
        memorySnapshot: null,
      },
      redacted: [],
      gitPatch: null,
    };

    const program = new Command().name("maestro").option("--json");
    registerBundleCommand(program);

    await program.parseAsync([
      "node",
      "maestro",
      "bundle",
      "inspect",
      "/tmp/does-not-matter.tar.gz",
    ]);

    expect(archive.readManifestCalls).toEqual(["/tmp/does-not-matter.tar.gz"]);
    expect(captured.logs[0]).toBe("Bundle abc-123");
    expect(captured.logs.some((line) => line.includes("0 handoff"))).toBe(true);
    expect(captured.logs.some((l) => l.includes("Memory:  (redacted)"))).toBe(true);
  });
});
