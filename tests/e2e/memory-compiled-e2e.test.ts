import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
} from "bun:test";
import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-memory-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("compiled memory feature E2E", () => {
  it(
    "runs the full memory lifecycle: correct -> learn -> recall -> search -> compile -> lint",
    async () => {
      const correct = await runCompiled(
        [
          "memory-correct",
          "use bun not npm",
          "--source",
          "ran npm install",
          "--trigger",
          "package,install,npm",
          "--severity",
          "hard",
          "--json",
        ],
        tmpDir,
      );
      expect(correct.exitCode).toBe(0);
      const correction = expectJson<{
        id: string;
        rule: string;
        severity: string;
        trigger: { keywords: string[] };
      }>(correct);
      expect(correction.id).toMatch(/^\d{4}-\d{2}-\d{2}-\d{3}$/);
      expect(correction.rule).toBe("use bun not npm");
      expect(correction.severity).toBe("hard");
      expect(correction.trigger.keywords).toEqual(["package", "install", "npm"]);

      const correctionsDir = join(tmpDir, ".maestro", "memory", "corrections");
      const files = await readdir(correctionsDir);
      expect(files).toContain(`${correction.id}.json`);
      const raw = await readFile(
        join(correctionsDir, `${correction.id}.json`),
        "utf8",
      );
      const parsed = JSON.parse(raw);
      expect(parsed.id).toBe(correction.id);
      expect(parsed.severity).toBe("hard");

      const stats1 = await runCompiled(["memory-stats"], tmpDir);
      expect(stats1.exitCode).toBe(0);
      expect(stats1.stdout).toContain("Corrections: 1");
      expect(stats1.stdout).toContain("1 hard");

      const learn = await runCompiled(
        [
          "memory-learn",
          "--content",
          "handoff context needs git diff",
          "--json",
        ],
        tmpDir,
      );
      expect(learn.exitCode).toBe(0);
      const learning = expectJson<{ content: string; sessionDate: string }>(
        learn,
      );
      expect(learning.content).toBe("handoff context needs git diff");
      expect(learning.sessionDate).toMatch(/^\d{4}-\d{2}-\d{2}$/);

      const learnDir = join(tmpDir, ".maestro", "memory", "learnings", "raw");
      const learnFiles = await readdir(learnDir);
      expect(learnFiles.length).toBe(1);

      const recall = await runCompiled(
        [
          "memory-recall",
          "--task",
          "install dependencies",
          "--json",
        ],
        tmpDir,
      );
      expect(recall.exitCode).toBe(0);
      const recalled = expectJson<{
        corrections: Array<{ id: string; rule: string }>;
      }>(recall);
      expect(recalled.corrections.length).toBeGreaterThanOrEqual(1);
      expect(recalled.corrections[0]?.id).toBe(correction.id);

      const search = await runCompiled(
        ["memory-search", "bun", "--json"],
        tmpDir,
      );
      expect(search.exitCode).toBe(0);
      const searched = expectJson<{
        corrections: Array<{ id: string }>;
        learnings: Array<{ content: string }>;
      }>(search);
      expect(searched.corrections.length).toBeGreaterThanOrEqual(1);
      expect(searched.corrections[0]?.id).toBe(correction.id);

      const compile = await runCompiled(
        [
          "memory-compile",
          "--summary",
          "Sprint recap: prefer bun over npm everywhere.",
          "--json",
        ],
        tmpDir,
      );
      expect(compile.exitCode).toBe(0);
      const compileResult = expectJson<{
        compiled: { compiledAt: string; summary: string; rawCount: number };
      }>(compile);
      expect(compileResult.compiled.summary).toContain("bun");
      expect(compileResult.compiled.compiledAt).toMatch(/^\d{4}-\d{2}-\d{2}T/);
      expect(compileResult.compiled.rawCount).toBeGreaterThanOrEqual(1);

      const compiledPath = join(
        tmpDir,
        ".maestro",
        "memory",
        "learnings",
        "_compiled.json",
      );
      const compiledRaw = await readFile(compiledPath, "utf8");
      expect(() => JSON.parse(compiledRaw)).not.toThrow();

      const lint = await runCompiled(["memory-lint"], tmpDir);
      expect(lint.exitCode).toBe(0);
      expect(lint.stdout).toContain("healthy");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "memory-stats on empty state reports zero counts",
    async () => {
      const stats = await runCompiled(["memory-stats", "--json"], tmpDir);
      expect(stats.exitCode).toBe(0);
      // The JSON shape is implementation-defined but must parse.
      expect(() => JSON.parse(stats.stdout)).not.toThrow();

      const text = await runCompiled(["memory-stats"], tmpDir);
      expect(text.exitCode).toBe(0);
      expect(text.stdout).toContain("Corrections: 0");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "memory-search returns empty lists when nothing matches",
    async () => {
      // Seed one correction so the store is non-empty. memory-correct
      // requires at least --trigger or --globs to be useful for recall.
      const correct = await runCompiled(
        [
          "memory-correct",
          "use await everywhere",
          "--trigger",
          "async,Promise",
          "--json",
        ],
        tmpDir,
      );
      expect(correct.exitCode).toBe(0);

      // Search for a totally unrelated term.
      const search = await runCompiled(
        ["memory-search", "zzz-no-match-zzz", "--json"],
        tmpDir,
      );
      expect(search.exitCode).toBe(0);
      const payload = expectJson<{
        corrections: unknown[];
        learnings: unknown[];
      }>(search);
      expect(payload.corrections).toEqual([]);
      expect(payload.learnings).toEqual([]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "memory-correct captures file globs when provided",
    async () => {
      const correct = await runCompiled(
        [
          "memory-correct",
          "no fire-and-forget promises",
          "--trigger",
          "async,Promise",
          "--globs",
          "*.ts,*.tsx",
          "--severity",
          "hard",
          "--json",
        ],
        tmpDir,
      );
      expect(correct.exitCode).toBe(0);
      const parsed = expectJson<{
        trigger: { keywords: string[]; fileGlobs: string[] };
        severity: string;
      }>(correct);
      expect(parsed.severity).toBe("hard");
      expect(parsed.trigger.keywords).toEqual(["async", "Promise"]);
      expect(parsed.trigger.fileGlobs).toEqual(["*.ts", "*.tsx"]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "memory-correct rejects missing rule argument",
    async () => {
      const result = await runCompiled(["memory-correct"], tmpDir);
      expect(result.exitCode).not.toBe(0);
      const combined = `${result.stdout}\n${result.stderr}`;
      // Commander reports "missing required argument" for missing positional.
      expect(combined.toLowerCase()).toContain("argument");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "memory-recall without any memory returns empty collections",
    async () => {
      const recall = await runCompiled(
        ["memory-recall", "--task", "anything", "--json"],
        tmpDir,
      );
      expect(recall.exitCode).toBe(0);
      const payload = expectJson<{ corrections: unknown[] }>(recall);
      expect(payload.corrections).toEqual([]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "memory-lint on a clean empty system reports healthy",
    async () => {
      const lint = await runCompiled(["memory-lint"], tmpDir);
      expect(lint.exitCode).toBe(0);
      expect(lint.stdout).toContain("healthy");
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
