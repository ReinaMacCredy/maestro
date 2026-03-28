import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsHandoffStoreAdapter } from "../../../src/adapters/handoff-store.adapter.js";
import type { Handoff } from "../../../src/domain/types.js";

let tmpDir: string;
let store: FsHandoffStoreAdapter;

const makeHandoff = (overrides: Partial<Handoff> = {}): Handoff => ({
  id: "2026-03-28-001",
  timestamp: "2026-03-28T12:00:00Z",
  message: "Test handoff",
  session: {
    agent: "claude-code",
    sessionId: "test-session-id",
    sourcePath: "/tmp/sessions/test",
    cassIndexed: true,
  },
  sitrep: "Everything is fine",
  quickstart: "Run: bun test",
  git: {
    branch: "main",
    recentCommits: ["abc1234 feat: test"],
    changedFiles: ["src/test.ts"],
    workingTreeClean: true,
    diffStat: "+10 -5",
  },
  ...overrides,
});

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-store-"));
  store = new FsHandoffStoreAdapter(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("FsHandoffStoreAdapter", () => {
  describe("create", () => {
    it("creates handoff files and returns ID", async () => {
      const handoff = makeHandoff();
      const id = await store.create(handoff);
      expect(id).toBe("2026-03-28-001");

      const handoffFile = Bun.file(
        join(tmpDir, ".maestro", "handoffs", "2026-03-28-001", "handoff.json"),
      );
      expect(await handoffFile.exists()).toBe(true);
    });

    it("creates envelope with pending status", async () => {
      await store.create(makeHandoff());
      const envelope = await store.get("2026-03-28-001");
      expect(envelope?.status).toBe("pending");
    });
  });

  describe("get", () => {
    it("returns undefined for non-existent handoff", async () => {
      const result = await store.get("does-not-exist");
      expect(result).toBeUndefined();
    });

    it("returns envelope for existing handoff", async () => {
      await store.create(makeHandoff());
      const envelope = await store.get("2026-03-28-001");
      expect(envelope?.handoff.message).toBe("Test handoff");
    });
  });

  describe("list", () => {
    it("returns empty array when no handoffs exist", async () => {
      const result = await store.list();
      expect(result).toEqual([]);
    });

    it("returns all handoffs sorted by timestamp descending", async () => {
      await store.create(
        makeHandoff({ id: "2026-03-28-001", timestamp: "2026-03-28T10:00:00Z" }),
      );
      await store.create(
        makeHandoff({ id: "2026-03-28-002", timestamp: "2026-03-28T14:00:00Z" }),
      );
      const result = await store.list();
      expect(result).toHaveLength(2);
      expect(result[0]!.handoff.id).toBe("2026-03-28-002");
    });

    it("filters by status", async () => {
      await store.create(makeHandoff({ id: "2026-03-28-001" }));
      await store.create(makeHandoff({ id: "2026-03-28-002" }));
      await store.updateStatus("2026-03-28-001", "picked-up", {
        pickedUpBy: "codex",
      });

      const pending = await store.list({ status: "pending" });
      expect(pending).toHaveLength(1);
      expect(pending[0]!.handoff.id).toBe("2026-03-28-002");
    });
  });

  describe("getLatestPending", () => {
    it("returns undefined when no pending handoffs", async () => {
      const result = await store.getLatestPending();
      expect(result).toBeUndefined();
    });

    it("returns the most recent pending handoff", async () => {
      await store.create(
        makeHandoff({ id: "2026-03-28-001", timestamp: "2026-03-28T10:00:00Z" }),
      );
      await store.create(
        makeHandoff({ id: "2026-03-28-002", timestamp: "2026-03-28T14:00:00Z" }),
      );
      const result = await store.getLatestPending();
      expect(result?.handoff.id).toBe("2026-03-28-002");
    });
  });

  describe("updateStatus", () => {
    it("updates status to picked-up with metadata", async () => {
      await store.create(makeHandoff());
      await store.updateStatus("2026-03-28-001", "picked-up", {
        pickedUpBy: "codex",
      });
      const envelope = await store.get("2026-03-28-001");
      expect(envelope?.status).toBe("picked-up");
      expect(envelope?.pickedUpBy).toBe("codex");
      expect(envelope?.pickedUpAt).toBeTruthy();
    });

    it("throws for non-existent handoff", async () => {
      await expect(
        store.updateStatus("nope", "picked-up"),
      ).rejects.toThrow("not found");
    });
  });
});
