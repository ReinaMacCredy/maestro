import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsReplyStoreAdapter } from "@/features/reply/adapters/fs-reply-store.adapter.js";
import type { WorkerReply } from "@/features/reply/domain/reply-types.js";

let tmpDir: string;
let store: FsReplyStoreAdapter;

const makeReply = (overrides: Partial<WorkerReply> = {}): WorkerReply => ({
  featureId: "f-42",
  outcome: "completed",
  writtenAt: "2026-04-13T05:00:00.000Z",
  writtenBy: "human",
  ...overrides,
});

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-reply-store-"));
  store = new FsReplyStoreAdapter(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("FsReplyStoreAdapter", () => {
  describe("write + get", () => {
    it("round-trips a minimal reply", async () => {
      const reply = makeReply();
      await store.write(reply);

      const loaded = await store.get(reply.featureId);
      expect(loaded).toEqual(reply);
    });

    it("round-trips a reply with notes and source", async () => {
      const reply = makeReply({ notes: "tests pass", source: "cli" });
      await store.write(reply);

      const loaded = await store.get(reply.featureId);
      expect(loaded?.notes).toBe("tests pass");
      expect(loaded?.source).toBe("cli");
    });

    it("overwrites an existing reply on re-write", async () => {
      await store.write(makeReply({ notes: "first" }));
      await store.write(makeReply({ notes: "second" }));

      const loaded = await store.get("f-42");
      expect(loaded?.notes).toBe("second");
    });

    it("rejects path-traversal feature ids", async () => {
      await expect(
        store.write(makeReply({ featureId: "../escape" })),
      ).rejects.toThrow();
    });
  });

  describe("get", () => {
    it("returns undefined for missing feature", async () => {
      const result = await store.get("never-written");
      expect(result).toBeUndefined();
    });

    it("returns undefined for malformed YAML (tolerant)", async () => {
      const dir = join(tmpDir, ".maestro", "replies");
      await mkdir(dir, { recursive: true });
      await writeFile(join(dir, "f-bad.yaml"), "outcome: !!not-a-valid-tag\n  : : :\n");

      const result = await store.get("f-bad");
      expect(result).toBeUndefined();
    });
  });

  describe("list", () => {
    it("returns empty array when the directory does not exist", async () => {
      const result = await store.list();
      expect(result).toEqual([]);
    });

    it("lists every valid reply sorted by writtenAt ascending", async () => {
      await store.write(makeReply({ featureId: "f-1", writtenAt: "2026-04-13T10:00:00.000Z" }));
      await store.write(makeReply({ featureId: "f-2", writtenAt: "2026-04-13T08:00:00.000Z" }));
      await store.write(makeReply({ featureId: "f-3", writtenAt: "2026-04-13T09:00:00.000Z" }));

      const list = await store.list();
      expect(list.map((r) => r.featureId)).toEqual(["f-2", "f-3", "f-1"]);
    });

    it("skips malformed yaml files", async () => {
      await store.write(makeReply({ featureId: "f-1" }));
      await writeFile(
        join(tmpDir, ".maestro", "replies", "f-bad.yaml"),
        "::: not really yaml :::\n",
      );
      await writeFile(
        join(tmpDir, ".maestro", "replies", "f-empty.yaml"),
        "",
      );

      const list = await store.list();
      expect(list.map((r) => r.featureId)).toEqual(["f-1"]);
    });
  });

  describe("listSince", () => {
    it("filters by ISO timestamp", async () => {
      await store.write(makeReply({ featureId: "f-1", writtenAt: "2026-04-13T08:00:00.000Z" }));
      await store.write(makeReply({ featureId: "f-2", writtenAt: "2026-04-13T10:00:00.000Z" }));

      const recent = await store.listSince("2026-04-13T09:00:00.000Z");
      expect(recent.map((r) => r.featureId)).toEqual(["f-2"]);
    });
  });

  describe("ingested marker", () => {
    it("reports not-ingested before markIngested is called", async () => {
      await store.write(makeReply());
      expect(await store.isIngested("f-42")).toBe(false);
    });

    it("reports ingested after markIngested", async () => {
      await store.write(makeReply());
      await store.markIngested("f-42");
      expect(await store.isIngested("f-42")).toBe(true);
    });

    it("markIngested is idempotent", async () => {
      await store.write(makeReply());
      await store.markIngested("f-42");
      await store.markIngested("f-42");
      expect(await store.isIngested("f-42")).toBe(true);
    });
  });
});
