import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
} from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  expectJson,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-bundle-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function writeBundlePlan(cwd: string): Promise<string> {
  const planPath = join(cwd, "bundle-plan.json");
  await writeFile(
    planPath,
    JSON.stringify(
      {
        title: "Bundle E2E Mission",
        description: "Exercise bundle export + inspect against the compiled CLI",
        milestones: [
          { id: "m1", title: "Build", description: "Core work", order: 0 },
        ],
        features: [
          {
            id: "f1",
            milestoneId: "m1",
            title: "Ship the core",
            description: "Do the thing",
            agentType: "implementer",
            verificationSteps: ["verify it"],
            fulfills: ["assertion-core-1"],
          },
        ],
      },
      null,
      2,
    ),
  );
  return planPath;
}

async function createMission(cwd: string): Promise<string> {
  const planPath = await writeBundlePlan(cwd);
  const result = await runCompiled(
    ["mission", "create", "--file", planPath, "--json"],
    cwd,
  );
  expect(result.exitCode).toBe(0);
  return JSON.parse(result.stdout).mission.id as string;
}

interface ExportManifest {
  schemaVersion: number;
  bundleId: string;
  createdAt: string;
  maestroVersion: string;
  mission: { id: string; title: string; status: string };
  stats: {
    features: number;
    milestones: number;
    assertions: number;
    workers: number;
    replies: number;
    handoffs: number;
    checkpoints: number;
    principlesSnapshot: number;
    outcomesSnapshot: number;
    memorySnapshot: { corrections: number; learnings: number } | null;
  };
  redacted: string[];
  gitPatch: { base: string; commits: number; bytes: number } | null;
}

interface ExportResult {
  manifest: ExportManifest;
  outputPath: string;
  bytes: number;
}

describe("compiled bundle feature E2E", () => {
  it(
    "exports a mission bundle and inspects its manifest",
    async () => {
      const missionId = await createMission(tmpDir);
      const outPath = join(tmpDir, "bundle.mission.tar.gz");

      const exportResult = await runCompiled(
        ["bundle", "export", missionId, "--out", outPath, "--json"],
        tmpDir,
      );
      expect(exportResult.exitCode).toBe(0);
      const exported = expectJson<ExportResult>(exportResult);
      expect(exported.manifest.schemaVersion).toBe(1);
      expect(exported.manifest.mission.id).toBe(missionId);
      expect(exported.manifest.stats.features).toBe(1);
      expect(exported.manifest.stats.milestones).toBe(1);
      expect(exported.manifest.redacted).toEqual([]);
      expect(exported.outputPath).toBe(outPath);
      expect(exported.bytes).toBeGreaterThan(0);

      const listing = await runCompiled(
        ["bundle", "inspect", outPath, "--json"],
        tmpDir,
      );
      expect(listing.exitCode).toBe(0);
      const manifest = expectJson<ExportManifest>(listing);
      expect(manifest.schemaVersion).toBe(1);
      expect(manifest.bundleId).toBe(exported.manifest.bundleId);
      expect(manifest.mission.id).toBe(missionId);
      expect(manifest.mission.title).toBe("Bundle E2E Mission");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "honors --redact and drops memory + prompts from the bundle",
    async () => {
      const missionId = await createMission(tmpDir);
      const outPath = join(tmpDir, "redacted.mission.tar.gz");

      const exportResult = await runCompiled(
        [
          "bundle",
          "export",
          missionId,
          "--out",
          outPath,
          "--redact",
          "memory,prompts",
          "--json",
        ],
        tmpDir,
      );
      expect(exportResult.exitCode).toBe(0);
      const exported = expectJson<ExportResult>(exportResult);
      expect(exported.manifest.redacted.sort()).toEqual(["memory", "prompts"]);
      expect(exported.manifest.stats.memorySnapshot).toBeNull();

      const inspect = await runCompiled(
        ["bundle", "inspect", outPath, "--json"],
        tmpDir,
      );
      expect(inspect.exitCode).toBe(0);
      const manifest = expectJson<ExportManifest>(inspect);
      expect(manifest.redacted.sort()).toEqual(["memory", "prompts"]);
      expect(manifest.stats.memorySnapshot).toBeNull();
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "rejects unknown --redact scopes with a helpful error",
    async () => {
      const missionId = await createMission(tmpDir);

      const result = await runCompiled(
        ["bundle", "export", missionId, "--redact", "memory,bogus"],
        tmpDir,
      );
      expect(result.exitCode).not.toBe(0);
      const combined = `${result.stdout}\n${result.stderr}`;
      expect(combined).toContain("bogus");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "falls back to a timestamped default output when --out is omitted",
    async () => {
      const missionId = await createMission(tmpDir);

      const result = await runCompiled(
        ["bundle", "export", missionId, "--json"],
        tmpDir,
      );
      expect(result.exitCode).toBe(0);
      const exported = expectJson<ExportResult>(result);
      expect(exported.outputPath).toMatch(
        new RegExp(`${missionId}-\\d{8}-\\d{6}\\.mission\\.tar\\.gz$`),
      );
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
