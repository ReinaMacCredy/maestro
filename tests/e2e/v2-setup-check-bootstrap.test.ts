import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, stat, writeFile } from "node:fs/promises";
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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-v2-setup-check-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

const V2_DIRS = [
  ".maestro/tasks",
  ".maestro/plans",
  ".maestro/evidence",
  ".maestro/runs",
  "docs/principles",
];

describe("maestro setup check (v2)", () => {
  it("reports missing directories with exit 1 on a fresh repo", async () => {
    const result = await runCompiled(["setup", "check"], tmpDir);
    expect(result.exitCode).toBe(1);
    for (const dir of V2_DIRS) expect(result.stdout).toContain(dir);
    expect(result.stdout).toContain("action required");
  });

  it("--json emits a machine-readable report", async () => {
    const result = await runCompiled(["setup", "check", "--json"], tmpDir);
    expect(result.exitCode).toBe(1);
    const parsed = JSON.parse(result.stdout) as {
      ok: boolean;
      entries: { path: string; status: string }[];
    };
    expect(parsed.ok).toBe(false);
    expect(parsed.entries.some((e) => e.path === ".maestro/tasks" && e.status === "missing")).toBe(true);
  });

  it("reports ok after bootstrap + a seeded principle file", async () => {
    await runCompiled(["setup", "bootstrap"], tmpDir);
    await writeFile(
      join(tmpDir, "docs/principles/example.md"),
      "# example\n## Rule\n\nx\n## Rationale\n\nx\n## Scan Command\n\n! rg x\n## Fix Recipe\n\nx\n",
      "utf8",
    );
    await mkdir(join(tmpDir, ".maestro"), { recursive: true });
    await writeFile(join(tmpDir, ".maestro/config.yaml"), "version: 1\n", "utf8");
    const result = await runCompiled(["setup", "check"], tmpDir);
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("setup check: OK");
  });
});

describe("maestro setup bootstrap (v2)", () => {
  it("creates every missing v2 directory and is idempotent", async () => {
    const first = await runCompiled(["setup", "bootstrap"], tmpDir);
    expect(first.exitCode).toBe(0);
    expect(first.stdout).toContain("created 5, skipped 0");
    for (const dir of V2_DIRS) {
      expect((await stat(join(tmpDir, dir))).isDirectory()).toBe(true);
    }
    const second = await runCompiled(["setup", "bootstrap"], tmpDir);
    expect(second.exitCode).toBe(0);
    expect(second.stdout).toContain("nothing to create");
  });

  it("--json emits the created/skipped lists", async () => {
    const result = await runCompiled(["setup", "bootstrap", "--json"], tmpDir);
    expect(result.exitCode).toBe(0);
    const parsed = JSON.parse(result.stdout) as {
      created: string[];
      skipped: string[];
    };
    expect(parsed.created.length).toBe(5);
    expect(parsed.skipped.length).toBe(0);
  });
});
