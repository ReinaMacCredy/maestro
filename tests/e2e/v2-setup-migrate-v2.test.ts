import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { cp, mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  buildCompiledCli,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

const FIXTURE = join(import.meta.dir, "..", "fixtures/v1-maestro");

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-v2-migrate-"));
  await initGitRepo(tmpDir);
  await cp(FIXTURE, tmpDir, { recursive: true });
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("maestro setup migrate-v2", () => {
  it("prints one row per step and exits 0", async () => {
    const result = await runCompiled(["setup", "migrate-v2"], tmpDir);
    expect(result.exitCode).toBe(0);
    for (const id of [
      "preflight",
      "backup",
      "bootstrap-dirs",
      "migrate-corrections",
      "migrate-tasks",
      "migrate-plans",
      "migrate-evidence",
      "migrate-policies",
      "seed-principles",
      "write-flag",
      "verify",
    ]) {
      expect(result.stdout).toContain(id);
    }
    expect(result.stdout).toContain("migrate-v2: complete");
  });

  it("creates a backup directory and the migration flag", async () => {
    await runCompiled(["setup", "migrate-v2"], tmpDir);
    const entries = await readdir(tmpDir);
    expect(entries.some((e) => e.startsWith(".maestro.backup-"))).toBe(true);
    const flag = await readFile(join(tmpDir, ".maestro/.migrated-v2.json"), "utf8");
    const parsed = JSON.parse(flag) as { version: number; steps: string[] };
    expect(parsed.version).toBe(1);
    expect(parsed.steps).toContain("preflight");
  });

  it("--dry-run skips backup and does not write the flag", async () => {
    const result = await runCompiled(["setup", "migrate-v2", "--dry-run"], tmpDir);
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("dry-run");
    const entries = await readdir(tmpDir);
    expect(entries.some((e) => e.startsWith(".maestro.backup-"))).toBe(false);
    const maestroFiles = await readdir(join(tmpDir, ".maestro"));
    expect(maestroFiles).not.toContain(".migrated-v2.json");
  });

  it("second run reports already-migrated unless --force", async () => {
    await runCompiled(["setup", "migrate-v2"], tmpDir);
    const second = await runCompiled(["setup", "migrate-v2"], tmpDir);
    expect(second.exitCode).toBe(0);
    expect(second.stdout).toContain("already migrated");
    const forced = await runCompiled(
      ["setup", "migrate-v2", "--force"],
      tmpDir,
    );
    expect(forced.exitCode).toBe(0);
    expect(forced.stdout).toContain("migrate-v2: complete");
  });

  it("--json emits the full step table", async () => {
    const result = await runCompiled(
      ["setup", "migrate-v2", "--json", "--dry-run"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    const parsed = JSON.parse(result.stdout) as {
      ok: boolean;
      dry_run: boolean;
      steps: { id: string; status: string }[];
    };
    expect(parsed.dry_run).toBe(true);
    expect(parsed.steps.length).toBe(11);
  });

  // Full end-to-end assertion table: a single run against the v1 fixture must
  // produce every artifact the v2 verbs depend on. Add a row here whenever a
  // step starts writing new files; the table protects future PRs from silently
  // breaking the migration contract.
  it("produces every expected artifact after one successful run", async () => {
    const result = await runCompiled(["setup", "migrate-v2", "--json"], tmpDir);
    expect(result.exitCode).toBe(0);

    // tasks.jsonl has 3 rows mirroring the v1 fixture
    const tasks = await readFile(
      join(tmpDir, ".maestro/tasks/tasks.jsonl"),
      "utf8",
    );
    const taskRows = tasks
      .trim()
      .split("\n")
      .map((l) => JSON.parse(l) as { id: string; state: string });
    expect(taskRows.length).toBe(3);
    expect(taskRows.map((r) => r.id).sort()).toEqual([
      "tsk-v1-claimed",
      "tsk-v1-draft",
      "tsk-v1-shipped",
    ]);

    // plans.jsonl has 1 row from .maestro/missions/mis-001/
    const plans = await readFile(
      join(tmpDir, ".maestro/plans/plans.jsonl"),
      "utf8",
    );
    const planRows = plans
      .trim()
      .split("\n")
      .map((l) => JSON.parse(l) as { id: string });
    expect(planRows.length).toBe(1);
    expect(planRows[0]!.id).toBe("mis-001");

    // docs/principles/legacy/<id>.md exists for each v1 correction
    const legacy = await readFile(
      join(tmpDir, "docs/principles/legacy/legacy-rule-1.md"),
      "utf8",
    );
    expect(legacy).toContain("Never paraphrase");

    // Default principles pack seeded
    const principlesDir = await readdir(join(tmpDir, "docs/principles"));
    expect(principlesDir).toContain("layer-order.md");
    expect(principlesDir).toContain("passive-harness.md");
    expect(principlesDir).toContain("prefer-shared-utils.md");
    expect(principlesDir).toContain("no-yolo-data-probing.md");

    // Verify step ran and ok=true
    const parsed = JSON.parse(result.stdout) as {
      ok: boolean;
      steps: { id: string; status: string }[];
    };
    expect(parsed.ok).toBe(true);
    const verify = parsed.steps.find((s) => s.id === "verify");
    expect(verify?.status).toBe("ok");
  });

  it("setup check exits 0 after migrate-v2", async () => {
    await runCompiled(["setup", "migrate-v2"], tmpDir);
    const check = await runCompiled(["setup", "check"], tmpDir);
    expect(check.exitCode).toBe(0);
    expect(check.stdout).toContain("setup check: OK");
  });
});
