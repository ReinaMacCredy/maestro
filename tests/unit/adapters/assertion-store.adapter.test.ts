import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, mkdir, writeFile } from "node:fs/promises";
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

const assertionsPathFor = (baseDir: string, currentMissionId: string): string =>
  join(baseDir, ".maestro", "missions", currentMissionId, "assertions.json");

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
      expect(assertion.result).toBe("pending");
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
      expect(data.assertions[0]!.id).toBe("a1");
    });

      it("creates directory if it doesn't exist", async () => {
        const input = makeCreateInput();
        await store.create(missionId, input, "a1");

        const dirPath = join(tmpDir, ".maestro", "missions", missionId);
        const { stat } = await import("node:fs/promises");
        expect((await stat(dirPath)).isDirectory()).toBe(true);
      });

      it("fails loudly and preserves invalid records already on disk", async () => {
        const filePath = assertionsPathFor(tmpDir, missionId);
        await mkdir(join(tmpDir, ".maestro", "missions", missionId), { recursive: true });
        await writeFile(
          filePath,
          `${JSON.stringify({
            assertions: [
              {
                id: "legacy-a1",
                missionId,
                milestoneId: "m1",
                featureId: "f1",
                result: "pending",
                description: "Legacy assertion missing timestamps",
                surface: "cli",
              },
            ],
          }, null, 2)}\n`,
        );
        const before = await Bun.file(filePath).text();

        await expect(store.create(missionId, makeCreateInput(), "a1")).rejects.toThrow(
          "Assertion store contains an invalid record",
        );

        expect(await Bun.file(filePath).text()).toBe(before);
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
      expect(assertion!.result).toBe("pending");
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
      const result = await store.update(missionId, "non-existent", { result: "passed" });
      expect(result).toBeUndefined();
    });

    it("updates assertion status to passed", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");

      const updated = await store.update(missionId, "a1", { result: "passed" });
      expect(updated!.result).toBe("passed");
      expect(updated!.updatedAt).toBeTruthy();
    });

    it("updates assertion status to failed with evidence", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");

      const updated = await store.update(missionId, "a1", {
        result: "failed",
        evidence: "Error message here",
      });
      expect(updated!.result).toBe("failed");
      expect(updated!.evidence).toBe("Error message here");
    });

    it("updates assertion status to waived with reason", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");

      const updated = await store.update(missionId, "a1", {
        result: "waived",
        waivedReason: "Not applicable for this feature",
      });
      expect(updated!.result).toBe("waived");
      expect(updated!.waivedReason).toBe("Not applicable for this feature");
    });

    it("preserves existing fields when updating", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");

      const updated = await store.update(missionId, "a1", { result: "passed" });
      expect(updated!.description).toBe("Test assertion description");
      expect(updated!.milestoneId).toBe("m1");
      expect(updated!.featureId).toBe("f1");
    });

    it("allows retry from failed to pending", async () => {
      const input = makeCreateInput();
      await store.create(missionId, input, "a1");
      await store.update(missionId, "a1", { result: "failed" });

      const updated = await store.update(missionId, "a1", { result: "pending" });
      expect(updated!.result).toBe("pending");
    });

      it("allows retry from blocked to pending", async () => {
        const input = makeCreateInput();
        await store.create(missionId, input, "a1");
        await store.update(missionId, "a1", { result: "blocked" });

        const updated = await store.update(missionId, "a1", { result: "pending" });
        expect(updated!.result).toBe("pending");
      });

      it("refuses to rewrite the file when another record is invalid", async () => {
        await store.create(missionId, makeCreateInput(), "a1");

        const filePath = assertionsPathFor(tmpDir, missionId);
        await writeFile(
          filePath,
          `${JSON.stringify({
            assertions: [
              {
                id: "a1",
                missionId,
                milestoneId: "m1",
                featureId: "f1",
                result: "pending",
                description: "Test assertion description",
                surface: "cli",
                createdAt: "2026-03-28T00:00:00.000Z",
                updatedAt: "2026-03-28T00:00:00.000Z",
              },
              {
                id: "legacy-a2",
                missionId,
                milestoneId: "m1",
                featureId: "f1",
                result: "pending",
                description: "Legacy assertion missing timestamps",
                surface: "cli",
              },
            ],
          }, null, 2)}\n`,
        );
        const before = await Bun.file(filePath).text();

        await expect(store.update(missionId, "a1", { result: "passed" })).rejects.toThrow(
          "Assertion store contains an invalid record",
        );

        expect(await Bun.file(filePath).text()).toBe(before);
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
      expect(assertions[0]!.id).toBe("a1");
      expect(assertions[1]!.id).toBe("a2");
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
      expect(assertions[0]!.id).toBe("a1");
    });
  });

  // ============================
  // Phase 7: Surface field roundtrip tests
  // ============================

  describe("surface field roundtrip", () => {
    it("roundtrips assertion with explicit surface", async () => {
      const input = makeCreateInput({ surface: "browser" });
      const created = await store.create(missionId, input, "a1");

      expect(created.surface).toBe("browser");

      // Read back from disk
      const loaded = await store.get(missionId, "a1");
      expect(loaded).toBeDefined();
      expect(loaded!.surface).toBe("browser");
    });

    it("defaults surface to cli when omitted", async () => {
      const input = makeCreateInput();
      const created = await store.create(missionId, input, "a1");

      expect(created.surface).toBe("cli");

      // Read back from disk
      const loaded = await store.get(missionId, "a1");
      expect(loaded).toBeDefined();
      expect(loaded!.surface).toBe("cli");
    });

    it("preserves surface through update", async () => {
      const input = makeCreateInput({ surface: "api" });
      await store.create(missionId, input, "a1");

      const updated = await store.update(missionId, "a1", { result: "passed" });
      expect(updated!.surface).toBe("api");
    });

    it("accepts custom surface values", async () => {
      const input = makeCreateInput({ surface: "e2e-cypress" });
      const created = await store.create(missionId, input, "a1");

      expect(created.surface).toBe("e2e-cypress");

      const loaded = await store.get(missionId, "a1");
      expect(loaded!.surface).toBe("e2e-cypress");
    });
  });
});
