import { afterEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  readUpdateCheckCache,
  writeUpdateCheckCache,
  resolveUpdateCheckCachePath,
} from "@/infra/adapters/update-check-cache.adapter.js";

const tempDirs: string[] = [];

async function makeTempPath(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "maestro-update-cache-"));
  tempDirs.push(dir);
  return join(dir, "update-check.json");
}

afterEach(async () => {
  await Promise.all(
    tempDirs.splice(0).map((dir) => rm(dir, { recursive: true, force: true })),
  );
});

describe("update check cache adapter", () => {
  it("returns undefined when the file does not exist", async () => {
    const path = await makeTempPath();
    expect(await readUpdateCheckCache(path)).toBeUndefined();
  });

  it("roundtrips an entry through write and read", async () => {
    const path = await makeTempPath();
    const entry = {
      checkedAt: "2026-04-26T12:00:00.000Z",
      currentVersion: "0.59.0",
      latestVersion: "0.60.0",
      latestTag: "v0.60.0",
    };
    await writeUpdateCheckCache(entry, path);
    expect(await readUpdateCheckCache(path)).toEqual(entry);
  });

  it("returns undefined for a malformed file rather than throwing", async () => {
    const path = await makeTempPath();
    await writeFile(path, "{not-json", "utf8");
    expect(await readUpdateCheckCache(path)).toBeUndefined();
  });

  it("returns undefined when required fields are missing", async () => {
    const path = await makeTempPath();
    await writeFile(path, JSON.stringify({ checkedAt: "x" }), "utf8");
    expect(await readUpdateCheckCache(path)).toBeUndefined();
  });

  it("creates the parent directory when writing for the first time", async () => {
    const dir = await mkdtemp(join(tmpdir(), "maestro-update-cache-"));
    tempDirs.push(dir);
    const nestedPath = join(dir, "nested", "subdir", "update-check.json");
    await writeUpdateCheckCache({
      checkedAt: "2026-04-26T12:00:00.000Z",
      currentVersion: "0.59.0",
      latestVersion: "0.59.0",
      latestTag: "v0.59.0",
    }, nestedPath);
    expect(await readUpdateCheckCache(nestedPath)).toBeDefined();
  });

  it("resolves a default path under ~/.maestro/", () => {
    const path = resolveUpdateCheckCachePath("/home/foo");
    expect(path).toContain(".maestro");
    expect(path).toContain("update-check.json");
  });
});
