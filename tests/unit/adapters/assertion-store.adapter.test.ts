import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsAssertionStoreAdapter } from "../../../src/adapters/assertion-store.adapter.js";
import type { CreateAssertionInput, Assertion } from "../../../src/domain/mission-types.js";

let tmpDir: string;
let store: FsAssertionStoreAdapter;
const missionId = "2026-03-28-001";

const makeCreateInput = (overrides: Partial<CreateAssertionInput> = {}): CreateAssertionInput => ({
  missionId,
  milestoneId: "m1",
  featureId: "f1",
  description: "Test assertion description",
  ...overrides,
});

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-assertion-store-"));
  store = new FsAssertionStoreAdapter(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("FsAssertionStoreAdapter", () => {
  describe("create", () => {
    it("creates an assertion and returns it", async () => {
      const input = makeCreateInput();
      const assertion = await store.create(missionId, input, "a1");

      expect(assertion.id).toBe("a1");
      expect(assertion.missionId).toBe(missionId);
      expect(assertion.status).toBe("pending");
      expect(assertion.description).toBe("Test assertion description");
    });

    it("persists assertion to assertions.json", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");

      // Verify file exists
      const filePath = join(tmpDir, ".maestro", "missions", missionId, "assertions.json");
      const file = Bun.file(filePath);
      expect(await file.exists()).toBe(true);

      const data = await file.json() as { assertions: Assertion[] };
      expect(data.assertions).toHaveLength(1);
      expect(data.assertions[0].id).toBe("a1");
    });

    it("creates directory if it doesn't exist", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");

      const dirPath = join(tmpDir, ".maestro", "missions", missionId);
      const { stat } = await import("node:fs/promises");
      expect((await stat(dirPath)).isDirectory()).toBe(true);
    });
  });

  describe("get", () => {
    it("returns undefined for non-existent assertion", async () => {
      const result = await store.get(missionId, "non-existent");
      expect(result).toBeUndefined();
    });

    it("returns assertion after creation", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");

      const assertion = await store.get(missionId, "a1");
      expect(assertion).toBeDefined();
      expect(assertion!.id).toBe("a1");
      expect(assertion!.status).toBe("pending");
    });

    it("returns correct assertion from multiple", async () => {
      await store.create(missionId, makeCreateInput(), "a1");
      await store.create(missionId, makeCreateInput({ description: "Second" }), "a2");

      const assertion = await store.get(missionId, "a2");
      expect(assertion!.description).toBe("Second");
    });
  });

  describe("exists", () => {
    it("returns false for non-existent assertion", async () => {
      const result = await store.exists(missionId, "non-existent");
      expect(result).toBe(false);
    });

    it("returns true for existing assertion", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");

      const result = await store.exists(missionId, "a1");
      expect(result).toBe(true);
    });
  });

  describe("update", () => {
    it("returns undefined for non-existent assertion", async () => {
      const result = await store.update(missionId, "non-existent", { status: "passed" });
      expect(result).toBeUndefined();
    });

    it("updates assertion status to passed", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");

      const updated = await store.update(missionId, "a1", { status: "passed" });
      expect(updated!.status).toBe("passed");
      expect(updated!.updatedAt).toBeTruthy();
    });

    it("updates assertion status to failed with evidence", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");

      const updated = await store.update(missionId, "a1", {
        status: "failed",
        evidence: "Error message here",
      });
      expect(updated!.status).toBe("failed");
      expect(updated!.evidence).toBe("Error message here");
    });

    it("updates assertion status to waived with reason", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");

      const updated = await store.update(missionId, "a1", {
        status: "waived",
        waivedReason: "Not applicable for this feature",
      });
      expect(updated!.status).toBe("waived");
      expect(updated!.waivedReason).toBe("Not applicable for this feature");
    });

    it("preserves existing fields when updating", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");

      const updated = await store.update(missionId, "a1", { status: "passed" });
      expect(updated!.description).toBe("Test assertion description");
      expect(updated!.milestoneId).toBe("m1");
      expect(updated!.featureId).toBe("f1");
    });

    it("allows retry from failed to pending", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");
      await store.update(missionId, "a1", { status: "failed" });

      const updated = await store.update(missionId, "a1", { status: "pending" });
      expect(updated?.status).toBe("pending");
    });

    it("allows retry from blocked to pending", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");
      await store.update(missionId, "a1", { status: "blocked" });

      const updated = await store.update(missionId, "a1", { status: "pending" });
      expect(updated?.status).toBe("pending");
    });
  });

  describe("list", () => {
    it("returns empty array when no assertions exist", async () => {
      const result = await store.list(missionId);
      expect(result).toEqual([]);
    });

    it("returns all assertions sorted by createdAt", async () => {
      await store.create(missionId, makeCreateInput(), "a1");
      await new Promise((r) => setTimeout(r, 10));
      await store.create(missionId, makeCreateInput(), "a2");

      const assertions = await store.list(missionId);
      expect(assertions).toHaveLength(2);
      expect(assertions[0].id).toBe("a1");
      expect(assertions[1].id).toBe("a2");
    });
  });

  describe("listByMilestone", () => {
    it("returns empty array when no assertions for milestone", async () => {
      await store.create(missionId, makeCreateInput({ milestoneId: "m1" }), "a1");

      const result = await store.listByMilestone(missionId, "m2");
      expect(result).toEqual([]);
    });

    it("returns only assertions for specified milestone", async () => {
      await store.create(missionId, makeCreateInput({ milestoneId: "m1" }), "a1");
      await store.create(missionId, makeCreateInput({ milestoneId: "m2" }), "a2");
      await store.create(missionId, makeCreateInput({ milestoneId: "m1" }), "a3");

      const assertions = await store.listByMilestone(missionId, "m1");
      expect(assertions).toHaveLength(2);
      expect(assertions.map((a) => a.id).sort()).toEqual(["a1", "a3"]);
    });
  });

  describe("getMany", () => {
    it("returns empty array for empty input", async () => {
      const result = await store.getMany(missionId, []);
      expect(result).toEqual([]);
    });

    it("returns multiple assertions by IDs", async () => {
      await store.create(missionId, makeCreateInput(), "a1");
      await store.create(missionId, makeCreateInput(), "a2");
      await store.create(missionId, makeCreateInput(), "a3");

      const assertions = await store.getMany(missionId, ["a1", "a3"]);
      expect(assertions).toHaveLength(2);
      expect(assertions.map((a) => a.id).sort()).toEqual(["a1", "a3"]);
    });

    it("skips non-existent assertion IDs", async () => {
      await store.create(missionId, makeCreateInput(), "a1");

      const assertions = await store.getMany(missionId, ["a1", "non-existent"]);
      expect(assertions).toHaveLength(1);
      expect(assertions[0].id).toBe("a1");
    });
  });
});
