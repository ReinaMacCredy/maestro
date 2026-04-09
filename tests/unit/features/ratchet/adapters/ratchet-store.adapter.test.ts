import { describe, it, expect, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsRatchetStoreAdapter } from "@/adapters/ratchet-store.adapter.js";

describe("FsRatchetStoreAdapter", () => {
  let dir: string;
  let store: FsRatchetStoreAdapter;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "maestro-ratchet-"));
    store = new FsRatchetStoreAdapter(dir);
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("returns empty suite when none exists", async () => {
    const suite = await store.getSuite();
    expect(suite.assertions).toEqual([]);
  });

  it("writes and reads suite", async () => {
    const suite = {
      assertions: [
        { id: "r1", correctionId: "c1", rule: "no npm", check: "npm install", createdAt: "2026-04-05" },
      ],
    };
    await store.writeSuite(suite);

    const read = await store.getSuite();
    expect(read.assertions.length).toBe(1);
    expect(read.assertions[0]!.rule).toBe("no npm");
  });

  it("returns undefined baseline when none exists", async () => {
    const baseline = await store.getBaseline();
    expect(baseline).toBeUndefined();
  });

  it("writes and reads baseline", async () => {
    const baseline = { passCount: 3, lastRunAt: "2026-04-05T10:00:00Z" };
    await store.writeBaseline(baseline);

    const read = await store.getBaseline();
    expect(read).toEqual(baseline);
  });
});
