import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { backupMaestroDir } from "@/service/migrate-v2-backup.js";

describe("backupMaestroDir", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-migrate-backup-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("returns created=false when .maestro/ does not exist", async () => {
    const result = await backupMaestroDir(root, "2026-05-15T10:00:00.000Z");
    expect(result.created).toBe(false);
  });

  it("copies the full .maestro/ tree into .maestro.backup-<sanitized-ts>/", async () => {
    await mkdir(join(root, ".maestro/nested"), { recursive: true });
    await writeFile(join(root, ".maestro/marker.txt"), "data", "utf8");
    await writeFile(join(root, ".maestro/nested/inner.txt"), "inner", "utf8");
    const result = await backupMaestroDir(root, "2026-05-15T10:00:00.000Z");
    expect(result.created).toBe(true);
    expect(result.destination).toContain(".maestro.backup-2026-05-15T10-00-00-000Z");
    const marker = await readFile(join(result.destination, "marker.txt"), "utf8");
    expect(marker).toBe("data");
    const inner = await readFile(join(result.destination, "nested/inner.txt"), "utf8");
    expect(inner).toBe("inner");
  });

  it("sanitizes ':' and '.' in the timestamp segment", async () => {
    await mkdir(join(root, ".maestro"), { recursive: true });
    const result = await backupMaestroDir(root, "2026-05-15T10:00:00.123Z");
    expect(result.destination).not.toContain(":");
    expect(result.destination).toContain("2026-05-15T10-00-00-123Z");
  });
});
