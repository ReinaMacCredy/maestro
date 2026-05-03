import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsFeatureStoreAdapter } from "@/features/mission/feature/adapters/feature-store.adapter.js";
import { migrateLegacyWorkerType } from "@/features/mission/feature/feature-migration.js";
import type { CreateFeatureInput, Feature, AgentReport } from "@/features/mission/domain/mission-types.js";

let tmpDir: string;
let store: FsFeatureStoreAdapter;
const missionId = "2026-03-28-001";

const makeCreateInput = (overrides: Partial<CreateFeatureInput> = {}): CreateFeatureInput => ({
  missionId,
  milestoneId: "m1",
  title: "Test Feature",
  description: "A test feature",
  agentType: "test-skill",
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

    it("rejects feature IDs with path traversal", async () => {
      await expect(
        store.create(missionId, makeCreateInput(), "../escape"),
      ).rejects.toThrow("Invalid feature ID");
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

    it("rejects feature IDs with path traversal on read", async () => {
      await expect(store.get(missionId, "../escape")).rejects.toThrow("Invalid feature ID");
    });

    it("transparently upgrades legacy workerType records on read", async () => {
      const now = new Date().toISOString();
      const legacyRecord = {
        id: "legacy",
        missionId,
        milestoneId: "m1",
        status: "pending",
        title: "Legacy",
        description: "",
        workerType: "codex-cli",
        verificationSteps: ["v"],
        dependsOn: [],
        fulfills: [],
        createdAt: now,
        updatedAt: now,
      };
      const path = join(tmpDir, ".maestro", "missions", missionId, "features", "legacy.json");
      await writeFile(path, JSON.stringify(legacyRecord, null, 2) + "\n", "utf8");

      const feature = await store.get(missionId, "legacy");
      expect(feature).toBeDefined();
      expect(feature!.agentType).toBe("codex-cli");

      // Disk copy should have been rewritten so the next read hits the new schema.
      const { readFile } = await import("node:fs/promises");
      const after = JSON.parse(await readFile(path, "utf8")) as Record<string, unknown>;
      expect(after.agentType).toBe("codex-cli");
      expect("workerType" in after).toBe(false);
    });
  });

  describe("migrateLegacyWorkerType", () => {
    it("renames workerType to agentType when agentType is absent", () => {
      const { normalized, migrated } = migrateLegacyWorkerType({
        id: "f1",
        workerType: "codex-cli",
      });
      expect(migrated).toBe(true);
      expect(normalized).toEqual({ id: "f1", agentType: "codex-cli" });
    });

    it("prefers existing agentType when both fields are present", () => {
      const { normalized, migrated } = migrateLegacyWorkerType({
        id: "f1",
        workerType: "legacy",
        agentType: "claude-code",
      });
      expect(migrated).toBe(true);
      expect(normalized).toEqual({ id: "f1", agentType: "claude-code" });
    });

    it("is a no-op when workerType is absent", () => {
      const input = { id: "f1", agentType: "codex-cli" };
      const { normalized, migrated } = migrateLegacyWorkerType(input);
      expect(migrated).toBe(false);
      expect(normalized).toBe(input);
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
      const result = await store.update(missionId, "non-existent", { status: "in-progress" });
      expect(result).toBeUndefined();
    });

    it("updates feature status", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "f1");

      const updated = await store.update(missionId, "f1", { status: "in-progress" });
      expect(updated!.status).toBe("in-progress");
      expect(updated!.updatedAt).toBeTruthy();
    });

    it("updates feature with agent report", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "f1");

      // Write with legacy format -- Zod transforms to rich format on read
      const legacyReport = {
        content: "Work completed successfully",
        timestamp: new Date().toISOString(),
        agent: "claude-code",
      };

      const report: AgentReport = {
        salientSummary: "Work completed successfully",
        whatWasImplemented: "Work completed successfully",
        whatWasLeftUndone: "",
        verification: { commandsRun: [], interactiveChecks: [] },
        tests: { added: [] },
        discoveredIssues: [],
      };

      await store.update(missionId, "f1", { report: legacyReport as unknown as AgentReport });
      // After round-trip through disk + Zod validation, expect rich format
      const reloaded = await store.get(missionId, "f1");
      expect(reloaded!.report).toEqual(report);
    });

    it("preserves existing fields when updating", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "f1");

      const updated = await store.update(missionId, "f1", { status: "in-progress" });
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
      expect(features.map((feature) => feature.id)).toEqual(["f1", "f2"]);
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
      await store.update(missionId, "f1", { status: "in-progress" });
      await store.create(missionId, makeCreateInput(), "f2");

      const features = await store.list(missionId, { status: "pending" });
      expect(features).toHaveLength(1);
      expect(features[0]!.id).toBe("f2");
    });

    it("combines filters", async () => {
      await store.create(missionId, makeCreateInput({ milestoneId: "m1" }), "f1");
      await store.update(missionId, "f1", { status: "in-progress" });
      await store.create(missionId, makeCreateInput({ milestoneId: "m2" }), "f2");
      await store.create(missionId, makeCreateInput({ milestoneId: "m1" }), "f3");

      const features = await store.list(missionId, { milestoneId: "m1", status: "in-progress" });
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
      expect(features[0]!.id).toBe("f1");
    });
  });

  // ============================
  // Phase 7: Field roundtrip tests
  // ============================

  describe("new field roundtrips", () => {
    it("roundtrips fulfills, preconditions, and expectedBehavior", async () => {
      const input = makeCreateInput({
        fulfills: ["assertion-1"],
        preconditions: "DB running",
        expectedBehavior: "Returns 200",
      });
      const created = await store.create(missionId, input, "f1");

      expect(created.fulfills).toEqual(["assertion-1"]);
      expect(created.preconditions).toBe("DB running");
      expect(created.expectedBehavior).toBe("Returns 200");

      // Read back from disk
      const loaded = await store.get(missionId, "f1");
      expect(loaded).toBeDefined();
      expect(loaded!.fulfills).toEqual(["assertion-1"]);
      expect(loaded!.preconditions).toBe("DB running");
      expect(loaded!.expectedBehavior).toBe("Returns 200");
    });

    it("defaults fulfills to empty array when omitted", async () => {
      const input = makeCreateInput();
      const created = await store.create(missionId, input, "f1");

      expect(created.fulfills).toEqual([]);

      const loaded = await store.get(missionId, "f1");
      expect(loaded!.fulfills).toEqual([]);
    });

    it("preserves new fields through update", async () => {
      const input = makeCreateInput({
        fulfills: ["assertion-1", "assertion-2"],
        preconditions: "Redis available",
        expectedBehavior: "Cache hit rate > 90%",
      });
      await store.create(missionId, input, "f1");

      // Update status -- new fields should survive
      const updated = await store.update(missionId, "f1", { status: "in-progress" });
      expect(updated!.fulfills).toEqual(["assertion-1", "assertion-2"]);
      expect(updated!.preconditions).toBe("Redis available");
      expect(updated!.expectedBehavior).toBe("Cache hit rate > 90%");
    });
  });
});
