import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsCheckpointStoreAdapter } from "../../../src/adapters/checkpoint-store.adapter.js";
import type { Checkpoint, FeatureStatus, AssertionResult } from "../../../src/domain/mission-types.js";

let tmpDir: string;
let store: FsCheckpointStoreAdapter;
const missionId = "2026-03-28-001";

const makeCheckpointData = (overrides: Partial<Omit<Checkpoint, "id">> = {}): Omit<Checkpoint, "id"> => ({
  missionId,
  currentMilestoneId: "m1",
  timestamp: new Date().toISOString(),
  featureStatuses: { f1: "pending" as FeatureStatus },
  assertionResults: { a1: "pending" as AssertionResult },
  ...overrides,
});

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-checkpoint-store-"));
  store = new FsCheckpointStoreAdapter(tmpDir);

  // Create the checkpoint directory structure
  await mkdir(join(tmpDir, ".maestro", "missions", missionId, "checkpoints"), { recursive: true });
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("FsCheckpointStoreAdapter", () => {
  describe("save", () => {
    it("saves a checkpoint and returns it with generated ID", async () => {
      const data = makeCheckpointData();
      const checkpoint = await store.save(missionId, data);

      expect(checkpoint.id).toBeTruthy();
      expect(checkpoint.missionId).toBe(missionId);
      expect(checkpoint.currentMilestoneId).toBe("m1");
      expect(checkpoint.timestamp).toBe(data.timestamp);
    });

    it("persists checkpoint to file with timestamp-based filename", async () => {
      const data = makeCheckpointData();
      const checkpoint = await store.save(missionId, data);

      // Verify file exists with timestamp pattern
      const dirPath = join(tmpDir, ".maestro", "missions", missionId, "checkpoints");
      const { readdir } = await import("node:fs/promises");
      const files = await readdir(dirPath);
      expect(files).toHaveLength(1);
      expect(files[0]).toMatch(/^\d{8}-\d{6}-\d{3}\.json$/);
    });

    it("captures feature states", async () => {
      const data = makeCheckpointData({
        featureStatuses: {
          f1: "in_progress",
          f2: "completed",
        },
      });
      const checkpoint = await store.save(missionId, data);

      expect(checkpoint.featureStatuses).toEqual({
        f1: "in_progress",
        f2: "completed",
      });
    });

    it("captures assertion states", async () => {
      const data = makeCheckpointData({
        assertionResults: {
          a1: "passed",
          a2: "failed",
          a3: "waived",
        },
      });
      const checkpoint = await store.save(missionId, data);

      expect(checkpoint.assertionResults).toEqual({
        a1: "passed",
        a2: "failed",
        a3: "waived",
      });
    });
  });

  describe("get", () => {
    it("returns undefined for non-existent checkpoint", async () => {
      const result = await store.get(missionId, "non-existent");
      expect(result).toBeUndefined();
    });

    it("returns checkpoint after saving", async () => {
      const data = makeCheckpointData();
      const saved = await store.save(missionId, data);

      const checkpoint = await store.get(missionId, saved.id);
      expect(checkpoint).toBeDefined();
      expect(checkpoint!.id).toBe(saved.id);
      expect(checkpoint!.currentMilestoneId).toBe("m1");
    });
  });

  describe("list", () => {
    it("returns empty array when no checkpoints exist", async () => {
      const result = await store.list(missionId);
      expect(result).toEqual([]);
    });

    it("returns all checkpoints sorted newest first", async () => {
      const now = new Date();
      const data1 = makeCheckpointData({ timestamp: new Date(now.getTime() - 1000).toISOString() });
      await store.save(missionId, data1);

      await new Promise((r) => setTimeout(r, 10));

      const data2 = makeCheckpointData({ timestamp: now.toISOString() });
      await store.save(missionId, data2);

      const checkpoints = await store.list(missionId);
      expect(checkpoints).toHaveLength(2);
      // Newest first
      expect(new Date(checkpoints[0]!.timestamp).getTime()).toBeGreaterThanOrEqual(
        new Date(checkpoints[1]!.timestamp).getTime(),
      );
    });
  });

  describe("getLatest", () => {
    it("returns undefined when no checkpoints exist", async () => {
      const result = await store.getLatest(missionId);
      expect(result).toBeUndefined();
    });

    it("returns the most recent checkpoint", async () => {
      const now = new Date();
      const data1 = makeCheckpointData({ timestamp: new Date(now.getTime() - 1000).toISOString() });
      await store.save(missionId, data1);

      await new Promise((r) => setTimeout(r, 10));

      const data2 = makeCheckpointData({ timestamp: now.toISOString() });
      const saved = await store.save(missionId, data2);

      const latest = await store.getLatest(missionId);
      expect(latest!.id).toBe(saved.id);
    });
  });

  describe("load", () => {
    it("returns undefined when no checkpoints exist", async () => {
      const result = await store.load(missionId);
      expect(result).toBeUndefined();
    });

    it("returns the latest checkpoint (alias for getLatest)", async () => {
      const data1 = makeCheckpointData();
      await store.save(missionId, data1);

      await new Promise((r) => setTimeout(r, 10));

      const data2 = makeCheckpointData();
      const saved = await store.save(missionId, data2);

      const loaded = await store.load(missionId);
      expect(loaded!.id).toBe(saved.id);
    });
  });

  describe("full checkpoint lifecycle", () => {
    it("can save, list, and retrieve checkpoints", async () => {
      // Save initial checkpoint
      const initialData = makeCheckpointData({
        currentMilestoneId: "m1",
        featureStatuses: { f1: "pending" },
        assertionResults: { a1: "pending" },
      });
      const cp1 = await store.save(missionId, initialData);

      // Progress some features and assertions
      await new Promise((r) => setTimeout(r, 10));
      const progressData = makeCheckpointData({
        currentMilestoneId: "m1",
        featureStatuses: { f1: "in_progress" },
        assertionResults: { a1: "passed" },
      });
      const cp2 = await store.save(missionId, progressData);

      // Verify list returns both, newest first
      const list = await store.list(missionId);
      expect(list).toHaveLength(2);
      expect(list[0]!.id).toBe(cp2.id);
      expect(list[1]!.id).toBe(cp1.id);

      // Verify get returns correct checkpoint
      const retrieved = await store.get(missionId, cp1.id);
      expect(retrieved!.featureStatuses).toEqual({ f1: "pending" });

      // Verify getLatest returns most recent
      const latest = await store.getLatest(missionId);
      expect(latest!.featureStatuses).toEqual({ f1: "in_progress" });
    });
  });
});
