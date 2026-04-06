import { describe, it, expect, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsLearningStoreAdapter } from "../../../src/adapters/learning-store.adapter.js";

describe("FsLearningStoreAdapter", () => {
  let dir: string;
  let store: FsLearningStoreAdapter;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "maestro-learn-"));
    store = new FsLearningStoreAdapter(dir);
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("appends and lists raw entries", async () => {
    await store.appendRaw({ sessionDate: "2026-04-05", content: "learning one" });
    await store.appendRaw({ sessionDate: "2026-04-05", content: "learning two", branch: "main" });

    const all = await store.listRaw();
    expect(all.length).toBe(2);
    expect(all[0].content).toBe("learning one");
    expect(all[1].content).toBe("learning two");
    expect(all[1].branch).toBe("main");
  });

  it("counts raw entries", async () => {
    expect(await store.rawCount()).toBe(0);

    await store.appendRaw({ sessionDate: "2026-04-05", content: "one" });
    await store.appendRaw({ sessionDate: "2026-04-05", content: "two" });

    expect(await store.rawCount()).toBe(2);
  });

  it("writes and reads compiled learnings", async () => {
    expect(await store.readCompiled()).toBeUndefined();

    const compiled = { compiledAt: "2026-04-05T10:00:00Z", summary: "key insights", rawCount: 5 };
    await store.writeCompiled(compiled);

    const read = await store.readCompiled();
    expect(read).toEqual(compiled);
  });

  it("returns empty list from empty store", async () => {
    const all = await store.listRaw();
    expect(all).toEqual([]);
  });
});
