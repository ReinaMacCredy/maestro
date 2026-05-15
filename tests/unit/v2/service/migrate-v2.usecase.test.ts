import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  migrateV2,
  MIGRATE_V2_STEP_TABLE,
} from "@/v2/service/migrate-v2.usecase.js";
import {
  MIGRATION_FLAG_REL,
  readMigrationFlag,
} from "@/v2/service/migrate-v2-flag.js";

const FIXTURE = join(__dirname, "../../../fixtures/v1-maestro");

describe("migrateV2 (scaffold)", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-migrate-"));
    await cp(FIXTURE, root, { recursive: true });
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("returns one entry per step in MIGRATE_V2_STEP_TABLE order", async () => {
    const result = await migrateV2({ repoRoot: root });
    expect(result.steps.length).toBe(MIGRATE_V2_STEP_TABLE.length);
    for (let i = 0; i < MIGRATE_V2_STEP_TABLE.length; i++) {
      expect(result.steps[i]!.id).toBe(MIGRATE_V2_STEP_TABLE[i]!.id);
    }
  });

  it("backs up .maestro/ to .maestro.backup-<ts>/ when not in dry-run", async () => {
    const FROZEN = new Date("2026-05-15T10:00:00.000Z");
    const result = await migrateV2({ repoRoot: root, clock: () => FROZEN });
    expect(result.backup?.created).toBe(true);
    const entries = await readdir(root);
    const backup = entries.find((e) => e.startsWith(".maestro.backup-"));
    expect(backup).toBeDefined();
    const taskFile = await readFile(
      join(root, backup!, "tasks/tasks.jsonl"),
      "utf8",
    );
    expect(taskFile).toContain("v1-demo-task");
  });

  it("creates missing v2 directories during bootstrap-dirs step", async () => {
    await migrateV2({ repoRoot: root });
    const runs = await readdir(join(root, ".maestro/runs"));
    expect(runs).toContain(".gitkeep");
    const principles = await readdir(join(root, "docs/principles"));
    expect(principles).toContain(".gitkeep");
  });

  it("writes .maestro/.migrated-v2.json with version, timestamp, and steps", async () => {
    const FROZEN = new Date("2026-05-15T10:30:00.000Z");
    await migrateV2({ repoRoot: root, clock: () => FROZEN });
    const flag = await readMigrationFlag(root);
    expect(flag).toBeDefined();
    expect(flag!.version).toBe(1);
    expect(flag!.migrated_at).toBe(FROZEN.toISOString());
    expect(flag!.steps).toContain("preflight");
    expect(flag!.steps).toContain("backup");
    expect(flag!.steps).toContain("bootstrap-dirs");
  });

  it("marks already_migrated and is a no-op on a second run", async () => {
    await migrateV2({ repoRoot: root });
    const second = await migrateV2({ repoRoot: root });
    expect(second.already_migrated).toBe(true);
    expect(second.steps.length).toBe(1);
    expect(second.steps[0]!.status).toBe("skipped");
  });

  it("re-runs every step when --force is passed even if flag is present", async () => {
    await migrateV2({ repoRoot: root });
    const forced = await migrateV2({ repoRoot: root }, { force: true });
    expect(forced.already_migrated).toBe(false);
    expect(forced.steps.length).toBe(MIGRATE_V2_STEP_TABLE.length);
  });

  it("dry-run skips backup, bootstrap-dirs, and flag-write", async () => {
    const result = await migrateV2({ repoRoot: root }, { dryRun: true });
    expect(result.dry_run).toBe(true);
    const stepById = new Map(result.steps.map((s) => [s.id, s]));
    expect(stepById.get("backup")?.status).toBe("skipped");
    expect(stepById.get("bootstrap-dirs")?.status).toBe("skipped");
    expect(stepById.get("write-flag")?.status).toBe("skipped");
    const flag = await readMigrationFlag(root);
    expect(flag).toBeUndefined();
  });

  it("emits not-implemented status for steps 4-9 and 11 in PR 32", async () => {
    const result = await migrateV2({ repoRoot: root });
    const pending = result.steps.filter((s) => s.status === "not-implemented");
    expect(pending.length).toBe(7);
    for (const step of pending) {
      expect(step.detail).toContain("PR 33");
    }
  });

  it("succeeds even when .maestro/ does not exist (greenfield repo)", async () => {
    const empty = await mkdtemp(join(tmpdir(), "v2-migrate-empty-"));
    try {
      const result = await migrateV2({ repoRoot: empty });
      expect(result.ok).toBe(true);
      expect(result.backup?.created).toBe(false);
      const flag = await readMigrationFlag(empty);
      expect(flag).toBeDefined();
    } finally {
      await rm(empty, { recursive: true, force: true });
    }
  });

  it("writes the flag at the canonical relative path", async () => {
    await migrateV2({ repoRoot: root });
    const raw = await readFile(join(root, MIGRATION_FLAG_REL), "utf8");
    expect(raw.trim().length).toBeGreaterThan(0);
  });

  it("captures the backup destination on the flag when backup was performed", async () => {
    await mkdir(join(root, ".maestro"), { recursive: true });
    await writeFile(join(root, ".maestro/marker.txt"), "v1", "utf8");
    const result = await migrateV2({ repoRoot: root });
    expect(result.flag?.backup_path).toContain(".maestro.backup-");
  });
});
