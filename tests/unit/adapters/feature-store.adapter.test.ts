import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsFeatureStoreAdapter } from "../../../src/adapters/feature-store.adapter.js";
import type { CreateFeatureInput, Feature, WorkerReport } from "../../../src/domain/mission-types.js";

let tmpDir: string;
let store: FsFeatureStoreAdapter;
const missionId = "2026-03-28-001";

const makeCreateInput = (overrides: Partial<CreateFeatureInput> = {}): CreateFeatureInput => ({
  missionId,
  milestoneId: "m1",
  title: "Test Feature",
  description: "A test feature",
  skillName: "test-skill",
  verificationSteps: ["step 1", "step 2"],
  ...overrides,
});

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-feature-store-"));
  store = new FsFeatureStoreAdapter(tmpDir);

  // Create the mission directory structure
  const { mkdir } = await import("node:fs/promises");
  await mkdir(join(tmpDir, ".maestro", "missions", missionId, "features"), { recursive: true });
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("FsFeatureStoreAdapter", () => {
  describe("create", () => {
    it("creates a feature and returns it", async () => {
      const input = makeCreateInput();
      const feature = await store.create(missionId, input, "f1");

      expect(feature.id).toBe("f1");
      expect(feature.missionId).toBe(missionId);
      expect(feature.status).toBe("pending");
      expect(feature.title).toBe("Test Feature");
      expect(feature.dependsOn).toEqual([]);
    });

    it("creates feature with dependencies", async () => {
      const input = makeCreateInput({ dependsOn: ["f0"] });
      const feature = await store.create(missionId, input, "f1");

      expect(feature.dependsOn).toEqual(["f0"]);
    });

    it("persists feature to file", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "f1");

      // Verify file exists
      const filePath = join(tmpDir, ".maestro", "missions", missionId, "features", "f1.json");
      const file = Bun.file(filePath);
      expect(await file.exists()).toBe(true);

      const data = await file.json() as Feature;
      expect(data.id).toBe("f1");
      expect(data.title).toBe("Test Feature");
    });
  });

  describe("get", () => {
    it("returns undefined for non-existent feature", async () => {
      const result = await store.get(missionId, "non-existent");
      expect(result).toBeUndefined();
    });

    it("returns feature after creation", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "f1");

      const feature = await store.get(missionId, "f1");
      expect(feature).toBeDefined();
      expect(feature!.id).toBe("f1");
      expect(feature!.status).toBe("pending");
    });
  });

  describe("exists", () => {
    it("returns false for non-existent feature", async () => {
      const result = await store.exists(missionId, "non-existent");
      expect(result).toBe(false);
    });

    it("returns true for existing feature", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "f1");

      const result = await store.exists(missionId, "f1");
      expect(result).toBe(true);
    });
  });

  describe("update", () => {
    it("returns undefined for non-existent feature", async () => {
      const result = await store.update(missionId, "non-existent", { status: "in_progress" });
      expect(result).toBeUndefined();
    });

    it("updates feature status", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "f1");

      const updated = await store.update(missionId, "f1", { status: "in_progress" });
      expect(updated!.status).toBe("in_progress");
      expect(updated!.updatedAt).toBeTruthy();
    });

    it("updates feature with worker report", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "f1");

      const report: WorkerReport = {
        content: "Work completed successfully",
        timestamp: new Date().toISOString(),
        agent: "claude-code",
      };

      const updated = await store.update(missionId, "f1", { report });
      expect(updated!.report).toEqual(report);
    });

    it("preserves existing fields when updating", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "f1");

      const updated = await store.update(missionId, "f1", { status: "in_progress" });
      expect(updated!.title).toBe("Test Feature");
      expect(updated!.description).toBe("A test feature");
      expect(updated!.verificationSteps).toEqual(["step 1", "step 2"]);
    });
  });

  describe("list", () => {
    it("returns empty array when no features exist", async () => {
      const result = await store.list(missionId);
      expect(result).toEqual([]);
    });

    it("returns all features for mission", async () => {
      await store.create(missionId, makeCreateInput(), "f1");
      await store.create(missionId, makeCreateInput({ title: "Second Feature" }), "f2");

      const features = await store.list(missionId);
      expect(features).toHaveLength(2);
    });

    it("filters by milestone", async () => {
      await store.create(missionId, makeCreateInput({ milestoneId: "m1" }), "f1");
      await store.create(missionId, makeCreateInput({ milestoneId: "m2" }), "f2");

      const features = await store.list(missionId, { milestoneId: "m1" });
      expect(features).toHaveLength(1);
      expect(features[0]!.id).toBe("f1");
    });

    it("filters by status", async () => {
      await store.create(missionId, makeCreateInput(), "f1");
      await store.update(missionId, "f1", { status: "in_progress" });
      await store.create(missionId, makeCreateInput(), "f2");

      const features = await store.list(missionId, { status: "pending" });
      expect(features).toHaveLength(1);
      expect(features[0]!.id).toBe("f2");
    });

    it("combines filters", async () => {
      await store.create(missionId, makeCreateInput({ milestoneId: "m1" }), "f1");
      await store.update(missionId, "f1", { status: "in_progress" });
      await store.create(missionId, makeCreateInput({ milestoneId: "m2" }), "f2");
      await store.create(missionId, makeCreateInput({ milestoneId: "m1" }), "f3");

      const features = await store.list(missionId, { milestoneId: "m1", status: "in_progress" });
      expect(features).toHaveLength(1);
      expect(features[0]!.id).toBe("f1");
    });
  });

  describe("getMany", () => {
    it("returns empty array for empty input", async () => {
      const result = await store.getMany(missionId, []);
      expect(result).toEqual([]);
    });

    it("returns multiple features by IDs", async () => {
      await store.create(missionId, makeCreateInput(), "f1");
      await store.create(missionId, makeCreateInput(), "f2");
      await store.create(missionId, makeCreateInput(), "f3");

      const features = await store.getMany(missionId, ["f1", "f3"]);
      expect(features).toHaveLength(2);
      expect(features.map((f) => f.id).sort()).toEqual(["f1", "f3"]);
    });

    it("skips non-existent feature IDs", async () => {
      await store.create(missionId, makeCreateInput(), "f1");

      const features = await store.getMany(missionId, ["f1", "non-existent"]);
      expect(features).toHaveLength(1);
      expect(features[0].id).toBe("f1");
    });
  });
});
