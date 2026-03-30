import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsMissionStoreAdapter } from "../../../src/adapters/mission-store.adapter.js";
import type { CreateMissionInput, Mission } from "../../../src/domain/mission-types.js";

let tmpDir: string;
let store: FsMissionStoreAdapter;

const makeCreateInput = (overrides: Partial<CreateMissionInput> = {}): CreateMissionInput => ({
  title: "Test Mission",
  description: "A test mission",
  milestones: [
    { id: "m1", title: "Milestone 1", description: "First milestone", order: 0 },
  ],
  ...overrides,
});

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-mission-store-"));
  store = new FsMissionStoreAdapter(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("FsMissionStoreAdapter", () => {
  describe("stage and finalize", () => {
    it("stages a mission and returns ID", async () => {
      const input = makeCreateInput();
      const id = await store.stage(input, "2026-03-28-001", []);
      expect(id).toBe("2026-03-28-001");
    });

    it("finalizes a staged mission into the active directory", async () => {
      const input = makeCreateInput();
      const id = await store.stage(input, "2026-03-28-001", []);
      await store.finalize(id);

      const mission = await store.get(id);
      expect(mission).toBeDefined();
      expect(mission!.status).toBe("draft");
      expect(mission!.title).toBe("Test Mission");
    });

    it("creates subdirectories on finalize", async () => {
      const input = makeCreateInput();
      const id = await store.stage(input, "2026-03-28-001", []);
      await store.finalize(id);

      // Check that feature, workers, and checkpoints dirs exist
      const featuresDir = join(tmpDir, ".maestro", "missions", id, "features");
      const workersDir = join(tmpDir, ".maestro", "missions", id, "workers");
      const checkpointsDir = join(tmpDir, ".maestro", "missions", id, "checkpoints");

      const { stat } = await import("node:fs/promises");
      expect((await stat(featuresDir)).isDirectory()).toBe(true);
      expect((await stat(workersDir)).isDirectory()).toBe(true);
      expect((await stat(checkpointsDir)).isDirectory()).toBe(true);
    });
  });

  describe("get", () => {
    it("returns undefined for non-existent mission", async () => {
      const result = await store.get("non-existent");
      expect(result).toBeUndefined();
    });

    it("returns mission after staging and finalizing", async () => {
      const input = makeCreateInput();
      const id = await store.stage(input, "2026-03-28-001", []);
      await store.finalize(id);

      const mission = await store.get(id);
      expect(mission).toBeDefined();
      expect(mission!.id).toBe(id);
      expect(mission!.milestones).toHaveLength(1);
    });
  });

  describe("exists", () => {
    it("returns false for non-existent mission", async () => {
      const result = await store.exists("non-existent");
      expect(result).toBe(false);
    });

    it("returns true for existing mission", async () => {
      const input = makeCreateInput();
      const id = await store.stage(input, "2026-03-28-001", []);
      await store.finalize(id);

      const result = await store.exists(id);
      expect(result).toBe(true);
    });
  });

  describe("listIds", () => {
    it("returns empty array when no missions exist", async () => {
      const result = await store.listIds();
      expect(result).toEqual([]);
    });

    it("returns all mission IDs sorted newest first", async () => {
      await store.stage(makeCreateInput(), "2026-03-28-001", []);
      await store.stage(makeCreateInput(), "2026-03-28-002", []);
      await store.finalize("2026-03-28-001");
      await store.finalize("2026-03-28-002");

      const ids = await store.listIds();
      expect(ids).toEqual(["2026-03-28-002", "2026-03-28-001"]);
    });
  });

  describe("list", () => {
    it("returns empty array when no missions exist", async () => {
      const result = await store.list();
      expect(result).toEqual([]);
    });

    it("returns all missions sorted by createdAt descending", async () => {
      const input1 = makeCreateInput();
      const id1 = await store.stage(input1, "2026-03-28-001", []);
      await store.finalize(id1);

      // Small delay to ensure different timestamps
      await new Promise((r) => setTimeout(r, 10));

      const input2 = makeCreateInput({ title: "Second Mission" });
      const id2 = await store.stage(input2, "2026-03-28-002", []);
      await store.finalize(id2);

      const missions = await store.list();
      expect(missions).toHaveLength(2);
      expect(missions[0]!.id).toBe(id2);
      expect(missions[1]!.id).toBe(id1);
    });
  });

  describe("update", () => {
    it("returns undefined for non-existent mission", async () => {
      const result = await store.update("non-existent", { status: "approved" });
      expect(result).toBeUndefined();
    });

    it("updates mission status and records timestamp", async () => {
      const input = makeCreateInput();
      const id = await store.stage(input, "2026-03-28-001", []);
      await store.finalize(id);

      const updated = await store.update(id, { status: "approved" });
      expect(updated!.status).toBe("approved");
      expect(updated!.approvedAt).toBeTruthy();
      expect(updated!.updatedAt).toBeTruthy();
    });

    it("updates mission title without changing status", async () => {
      const input = makeCreateInput();
      const id = await store.stage(input, "2026-03-28-001", []);
      await store.finalize(id);

      const updated = await store.update(id, { title: "Updated Title" });
      expect(updated!.title).toBe("Updated Title");
      expect(updated!.status).toBe("draft");
    });

    it("updates mission description", async () => {
      const input = makeCreateInput();
      const id = await store.stage(input, "2026-03-28-001", []);
      await store.finalize(id);

      const updated = await store.update(id, { description: "Updated description" });
      expect(updated!.description).toBe("Updated description");
    });

    it("records rejectedAt when transitioning to rejected", async () => {
      const input = makeCreateInput();
      const id = await store.stage(input, "2026-03-28-001", []);
      await store.finalize(id);

      const updated = await store.update(id, { status: "rejected" });
      expect(updated!.status).toBe("rejected");
      expect(updated!.rejectedAt).toBeTruthy();
    });

    it("records completedAt when transitioning to completed", async () => {
      const input = makeCreateInput();
      const id = await store.stage(input, "2026-03-28-001", []);
      await store.finalize(id);
      await store.update(id, { status: "approved" });
      await store.update(id, { status: "executing" });
      await store.update(id, { status: "validating" });

      const updated = await store.update(id, { status: "completed" });
      expect(updated!.status).toBe("completed");
      expect(updated!.completedAt).toBeTruthy();
    });
  });

  // ============================
  // Phase 7: Proposal field roundtrip test
  // ============================

  describe("proposal field roundtrip", () => {
    it("roundtrips mission with proposal through stage and finalize", async () => {
      const input = makeCreateInput({ proposal: "# Full proposal\n\nDetailed plan here." });
      const id = await store.stage(input, "2026-03-28-001", []);
      await store.finalize(id);

      const mission = await store.get(id);
      expect(mission).toBeDefined();
      expect(mission!.proposal).toBe("# Full proposal\n\nDetailed plan here.");
    });

    it("preserves proposal through update", async () => {
      const input = makeCreateInput({ proposal: "# Proposal v1" });
      const id = await store.stage(input, "2026-03-28-001", []);
      await store.finalize(id);

      // Update status -- proposal should survive
      const updated = await store.update(id, { status: "approved" });
      expect(updated!.proposal).toBe("# Proposal v1");
    });

    it("mission without proposal has undefined proposal field", async () => {
      const input = makeCreateInput();
      const id = await store.stage(input, "2026-03-28-001", []);
      await store.finalize(id);

      const mission = await store.get(id);
      expect(mission).toBeDefined();
      expect(mission!.proposal).toBeUndefined();
    });
  });
});
