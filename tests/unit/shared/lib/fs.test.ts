import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { writeText } from "@/shared/lib/fs.js";

describe("writeAtomic (via writeText)", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "maestro-fs-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("does not leave a .tmp.<uuid> sibling after a successful write", async () => {
    const target = join(dir, "data.json");
    await writeText(target, "x");
    const entries = await readdir(dir);
    expect(entries).toEqual(["data.json"]);
  });

  it("cleans up the tmp file when rename fails", async () => {
    // Writing under a path whose parent is a file (not a dir) makes rename fail.
    // Confirm we don't leave the tmp file behind.
    const blockerPath = join(dir, "blocker");
    await writeText(blockerPath, "blocker is a file, not a dir");
    const target = join(dir, "blocker", "data.json");
    let threw = false;
    try {
      await writeText(target, "x");
    } catch {
      threw = true;
    }
    expect(threw).toBe(true);
    // The blocker is still here; no tmp.<uuid> remnants next to it.
    const entries = await readdir(dir);
    expect(entries.filter((name) => name.includes(".tmp."))).toEqual([]);
  });
});
