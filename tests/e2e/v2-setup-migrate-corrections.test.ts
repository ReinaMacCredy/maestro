import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  buildCompiledCli,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-v2-migrate-corr-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function seedV1Correction(dir: string, id: string, body: object): Promise<void> {
  const d = join(dir, ".maestro/memory/corrections");
  await mkdir(d, { recursive: true });
  await writeFile(join(d, `${id}.json`), JSON.stringify(body), "utf8");
}

describe("maestro setup migrate-corrections (v2)", () => {
  it("is a no-op when no corrections directory exists", async () => {
    const result = await runCompiled(["setup", "migrate-corrections"], tmpDir);
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("nothing to migrate");
  });

  it("migrates v1 corrections to docs/principles/legacy/<id>.md", async () => {
    await seedV1Correction(tmpDir, "correction-001", {
      id: "correction-001",
      rule: "Never paraphrase user authorization.",
      source: "manual-edit",
      trigger: { keywords: ["paraphrase"], fileGlobs: ["src/**"] },
      severity: "hard",
      createdAt: "2026-04-01T00:00:00Z",
      updatedAt: "2026-04-01T00:00:00Z",
    });

    const result = await runCompiled(["setup", "migrate-corrections"], tmpDir);
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("scanned 1, migrated 1, skipped 0");
    expect(result.stdout).toContain("migrated correction-001");

    const md = await readFile(
      join(tmpDir, "docs/principles/legacy/correction-001.md"),
      "utf8",
    );
    expect(md).toContain("# correction-001");
    expect(md).toContain("Never paraphrase user authorization");
    expect(md).toContain("hard correction");
  });

  it("--json emits a machine-readable result", async () => {
    await seedV1Correction(tmpDir, "c-a", {
      id: "c-a",
      rule: "r",
      source: "s",
      trigger: { keywords: [], fileGlobs: [] },
      severity: "soft",
      createdAt: "2026-04-01T00:00:00Z",
      updatedAt: "2026-04-01T00:00:00Z",
    });
    const result = await runCompiled(
      ["setup", "migrate-corrections", "--json"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    const parsed = JSON.parse(result.stdout) as {
      scanned: number;
      migrated: string[];
      skipped: string[];
      missing_source: boolean;
    };
    expect(parsed.scanned).toBe(1);
    expect(parsed.migrated).toEqual(["c-a"]);
    expect(parsed.missing_source).toBe(false);
  });

  it("skips already-migrated corrections on a second run", async () => {
    await seedV1Correction(tmpDir, "c-1", {
      id: "c-1",
      rule: "rule",
      source: "src",
      trigger: { keywords: [], fileGlobs: [] },
      severity: "soft",
      createdAt: "2026-04-01T00:00:00Z",
      updatedAt: "2026-04-01T00:00:00Z",
    });
    await runCompiled(["setup", "migrate-corrections"], tmpDir);
    const second = await runCompiled(["setup", "migrate-corrections"], tmpDir);
    expect(second.exitCode).toBe(0);
    expect(second.stdout).toContain("migrated 0, skipped 1");
  });
});
