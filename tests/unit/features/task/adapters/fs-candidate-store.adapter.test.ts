import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsCandidateStoreAdapter } from "@/features/task/adapters/fs-candidate-store.adapter.js";

describe("FsCandidateStoreAdapter", () => {
  let tmpDir: string;
  let store: FsCandidateStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-candidate-"));
    store = new FsCandidateStoreAdapter(tmpDir);
  });

  describe("all", () => {
    it("returns empty array on a fresh store (directory does not exist)", async () => {
      expect(await store.all()).toEqual([]);
    });
  });

  describe("create", () => {
    it("persists a candidate with a capturedAt timestamp", async () => {
      const candidate = await store.create({
        id: "tsk-abc123",
        sourceTaskId: "tsk-abc123",
        title: "Implement argon2 password hashing",
        reason: "argon2 compare was backwards",
        keywords: ["argon2", "password", "hashing", "compare", "backwards"],
      });

      expect(candidate.id).toBe("tsk-abc123");
      expect(candidate.sourceTaskId).toBe("tsk-abc123");
      expect(candidate.sourceType).toBe("task-close");
      expect(candidate.title).toBe("Implement argon2 password hashing");
      expect(candidate.reason).toBe("argon2 compare was backwards");
      expect(candidate.keywords).toEqual([
        "argon2",
        "password",
        "hashing",
        "compare",
        "backwards",
      ]);
      expect(candidate.capturedAt).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    });

    it("persists across instances (round trip via disk)", async () => {
      await store.create({
        id: "tsk-a1b2c3",
        sourceTaskId: "tsk-a1b2c3",
        title: "Persistence test",
        reason: "reason",
        keywords: ["persist"],
      });

      const fresh = new FsCandidateStoreAdapter(tmpDir);
      const all = await fresh.all();
      expect(all.length).toBe(1);
      expect(all[0]?.id).toBe("tsk-a1b2c3");
    });
  });

  describe("all (after creates)", () => {
    it("returns every created candidate, unordered", async () => {
      await store.create({
        id: "tsk-aaaaaa",
        sourceTaskId: "tsk-aaaaaa",
        title: "A",
        reason: "a",
        keywords: ["a"],
      });
      await store.create({
        id: "tsk-bbbbbb",
        sourceTaskId: "tsk-bbbbbb",
        title: "B",
        reason: "b",
        keywords: ["b"],
      });
      await store.create({
        id: "tsk-cccccc",
        sourceTaskId: "tsk-cccccc",
        title: "C",
        reason: "c",
        keywords: ["c"],
      });

      const all = await store.all();
      const ids = all.map((c) => c.id).sort();
      expect(ids).toEqual(["tsk-aaaaaa", "tsk-bbbbbb", "tsk-cccccc"]);
    });

    it("silently skips non-json entries in the directory", async () => {
      // Write a stray file into the candidates dir to simulate a user
      // dropping garbage in there. The adapter should not crash.
      await store.create({
        id: "tsk-abcdef",
        sourceTaskId: "tsk-abcdef",
        title: "real",
        reason: "r",
        keywords: ["real"],
      });
      const candidatesDir = join(tmpDir, ".maestro", "tasks", "candidates");
      await Bun.write(join(candidatesDir, "not-json.txt"), "garbage");

      const all = await store.all();
      expect(all.length).toBe(1);
      expect(all[0]?.id).toBe("tsk-abcdef");
    });

    it("skips malformed candidate json instead of failing the whole read", async () => {
      await store.create({
        id: "tsk-abcdef",
        sourceTaskId: "tsk-abcdef",
        title: "real",
        reason: "r",
        keywords: ["real"],
      });
      const candidatesDir = join(tmpDir, ".maestro", "tasks", "candidates");
      await Bun.write(join(candidatesDir, "broken.json"), "{bad json\n");

      const all = await store.all();
      expect(all.map((candidate) => candidate.id)).toEqual(["tsk-abcdef"]);
    });

    it("ignores nested directories in the candidates folder", async () => {
      await store.create({
        id: "tsk-abcdef",
        sourceTaskId: "tsk-abcdef",
        title: "real",
        reason: "r",
        keywords: ["real"],
      });
      const candidatesDir = join(tmpDir, ".maestro", "tasks", "candidates");
      await mkdir(join(candidatesDir, "nested"), { recursive: true });

      const all = await store.all();
      expect(all.map((candidate) => candidate.id)).toEqual(["tsk-abcdef"]);
    });
  });

  describe("delete", () => {
    it("removes the candidate file", async () => {
      await store.create({
        id: "tsk-abcdef",
        sourceTaskId: "tsk-abcdef",
        title: "victim",
        reason: "r",
        keywords: ["v"],
      });

      await store.delete("tsk-abcdef");

      expect(await store.all()).toEqual([]);
    });

    it("tolerates missing candidates without throwing", async () => {
      await expect(store.delete("tsk-absent")).resolves.toBeUndefined();
    });
  });

  describe("path safety", () => {
    it("rejects traversal segments in the candidate id on create", async () => {
      await expect(
        store.create({
          id: "../../etc/passwd",
          sourceTaskId: "tsk-abcdef",
          title: "bad",
          reason: "r",
          keywords: ["r"],
        }),
      ).rejects.toThrow(/Invalid candidate id/);
    });

    it("rejects traversal segments in the candidate id on delete", async () => {
      await expect(store.delete("../etc/passwd")).rejects.toThrow(/Invalid candidate id/);
    });

    it("rejects path separators in the candidate id", async () => {
      await expect(
        store.create({
          id: "nested/foo",
          sourceTaskId: "tsk-abcdef",
          title: "bad",
          reason: "r",
          keywords: ["r"],
        }),
      ).rejects.toThrow(/Invalid candidate id/);
    });
  });
});
