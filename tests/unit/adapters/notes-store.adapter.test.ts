import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsNotesStoreAdapter } from "@/adapters/notes-store.adapter.js";

let tmpDir: string;
let store: FsNotesStoreAdapter;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-notes-store-"));
  store = new FsNotesStoreAdapter(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("FsNotesStoreAdapter", () => {
  it("returns an empty list when notes file does not exist", async () => {
    const notes = await store.list();
    expect(notes).toEqual([]);
  });

  it("appends notes to .maestro/notes.json", async () => {
    await store.append({
      timestamp: "2026-03-28T12:00:00Z",
      content: "First note",
      git_branch: "main",
    });
    await store.append({
      timestamp: "2026-03-28T12:05:00Z",
      content: "Second note",
      git_branch: "feature/test",
    });

    const notes = await store.list();
    expect(notes).toHaveLength(2);
    expect(notes[0]!.content).toBe("First note");
    expect(notes[1]!.git_branch).toBe("feature/test");

    const file = Bun.file(join(tmpDir, ".maestro", "notes.json"));
    expect(await file.exists()).toBe(true);
  });
});
