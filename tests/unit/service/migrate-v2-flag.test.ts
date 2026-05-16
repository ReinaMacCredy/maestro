import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  MIGRATION_FLAG_VERSION,
  readMigrationFlag,
  writeMigrationFlag,
} from "@/service/migrate-v2-flag.js";

describe("migrate-v2 flag", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-migrate-flag-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("returns undefined when the flag file is missing", async () => {
    expect(await readMigrationFlag(root)).toBeUndefined();
  });

  it("round-trips a flag through write then read", async () => {
    await writeMigrationFlag(root, {
      version: MIGRATION_FLAG_VERSION,
      migrated_at: "2026-05-15T10:00:00.000Z",
      steps: ["preflight", "backup"],
      backup_path: "/tmp/x",
    });
    const flag = await readMigrationFlag(root);
    expect(flag?.version).toBe(MIGRATION_FLAG_VERSION);
    expect(flag?.migrated_at).toBe("2026-05-15T10:00:00.000Z");
    expect(flag?.steps).toEqual(["preflight", "backup"]);
    expect(flag?.backup_path).toBe("/tmp/x");
  });

  it("creates the parent .maestro/ directory if it does not exist", async () => {
    await writeMigrationFlag(root, {
      version: 1,
      migrated_at: "now",
      steps: [],
    });
    const flag = await readMigrationFlag(root);
    expect(flag).toBeDefined();
  });
});
