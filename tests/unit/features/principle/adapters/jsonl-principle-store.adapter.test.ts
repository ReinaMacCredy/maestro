import { describe, it, expect, beforeEach } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { JsonlPrincipleStoreAdapter } from "@/features/principle/adapters/jsonl-principle-store.adapter.js";
import { ensureDir, writeText, readText } from "@/shared/lib/fs.js";
import type { CreatePrincipleInput } from "@/features/principle/domain/types.js";

let tempDir: string;
let adapter: JsonlPrincipleStoreAdapter;

beforeEach(async () => {
  tempDir = await mkdtemp(join(tmpdir(), "principle-store-test-"));
  await ensureDir(join(tempDir, ".maestro"));
  adapter = new JsonlPrincipleStoreAdapter(tempDir);
});

const ADVISORY_INPUT: CreatePrincipleInput = {
  id: "test-advisory",
  name: "Test Advisory",
  rule: "Be advisory",
  profiles: ["implementation"],
  mode: "advisory",
};

const GATE_INPUT: CreatePrincipleInput = {
  id: "test-gate",
  name: "Test Gate",
  source: "karpathy",
  rule: "Must gate",
  profiles: ["implementation", "planning"],
  mode: "gate",
  gateField: "assumptions",
  gateCheck: "array_min_length:1",
};

describe("JsonlPrincipleStoreAdapter", () => {
  describe("list", () => {
    it("returns empty array when file does not exist", async () => {
      const result = await adapter.list();
      expect(result).toEqual([]);
    });

    it("returns empty array for empty file", async () => {
      await writeText(join(tempDir, ".maestro", "principles.jsonl"), "");
      const result = await adapter.list();
      expect(result).toEqual([]);
    });
  });

  describe("create", () => {
    it("creates an advisory principle", async () => {
      const created = await adapter.create(ADVISORY_INPUT);
      expect(created.id).toBe("test-advisory");
      expect(created.source).toBe("custom");
      expect(created.mode).toBe("advisory");
    });

    it("creates a gate principle", async () => {
      const created = await adapter.create(GATE_INPUT);
      expect(created.gateField).toBe("assumptions");
      expect(created.gateCheck).toBe("array_min_length:1");
      expect(created.source).toBe("karpathy");
    });

    it("persists to disk and survives re-read", async () => {
        await adapter.create(ADVISORY_INPUT);
        const fresh = new JsonlPrincipleStoreAdapter(tempDir);
        const all = await fresh.list();
        expect(all).toHaveLength(1);
        expect(all[0]?.id).toBe("test-advisory");
    });

    it("throws on duplicate id", async () => {
      await adapter.create(ADVISORY_INPUT);
      await expect(adapter.create(ADVISORY_INPUT)).rejects.toThrow("already exists");
    });
  });

  describe("get", () => {
    it("returns principle by id", async () => {
      await adapter.create(ADVISORY_INPUT);
      const found = await adapter.get("test-advisory");
      expect(found).toBeDefined();
      expect(found!.name).toBe("Test Advisory");
    });

    it("returns undefined for missing id", async () => {
      const found = await adapter.get("nonexistent");
      expect(found).toBeUndefined();
    });
  });

  describe("listByProfile", () => {
    it("filters by profile", async () => {
      await adapter.create(ADVISORY_INPUT);
      await adapter.create(GATE_INPUT);

      const implResults = await adapter.listByProfile("implementation");
      expect(implResults).toHaveLength(2);

        const planResults = await adapter.listByProfile("planning");
        expect(planResults).toHaveLength(1);
        expect(planResults[0]?.id).toBe("test-gate");

      const reviewResults = await adapter.listByProfile("code-review");
      expect(reviewResults).toHaveLength(0);
    });
  });

  describe("remove", () => {
    it("removes existing principle and returns true", async () => {
      await adapter.create(ADVISORY_INPUT);
      const removed = await adapter.remove("test-advisory");
      expect(removed).toBe(true);
      const all = await adapter.list();
      expect(all).toHaveLength(0);
    });

    it("returns false for missing id", async () => {
      const removed = await adapter.remove("nonexistent");
      expect(removed).toBe(false);
    });
  });

  describe("corrupt line handling", () => {
    it("throws on corrupt JSON lines", async () => {
      const content = [
        JSON.stringify({
          id: "valid",
          name: "Valid",
          source: "custom",
          rule: "Rule",
          profiles: ["implementation"],
          mode: "advisory",
        }),
        "not json at all",
        JSON.stringify({
          id: "also-valid",
          name: "Also Valid",
          source: "custom",
          rule: "Rule 2",
          profiles: ["planning"],
          mode: "advisory",
        }),
      ].join("\n") + "\n";

      await writeText(join(tempDir, ".maestro", "principles.jsonl"), content);
      await expect(adapter.list()).rejects.toThrow("Invalid principle record at line 2");
    });

    it("throws on lines that fail validation", async () => {
      const content = [
        JSON.stringify({
          id: "valid",
          name: "Valid",
          source: "custom",
          rule: "Rule",
          profiles: ["implementation"],
          mode: "advisory",
        }),
        JSON.stringify({ id: "INVALID_UPPERCASE", invalid: true }),
      ].join("\n") + "\n";

      await writeText(join(tempDir, ".maestro", "principles.jsonl"), content);
      await expect(adapter.list()).rejects.toThrow("Invalid principle schema at line 2");
    });
  });

  describe("recordOutcome + listOutcomes", () => {
    it("returns empty list when no outcomes file exists", async () => {
      expect(await adapter.listOutcomes()).toEqual([]);
    });

    it("appends records to outcomes.jsonl", async () => {
      await adapter.recordOutcome({
        principleId: "p-1",
        handoffId: "2026-04-13-001",
        outcome: "pending",
        recordedAt: "2026-04-13T00:00:00.000Z",
      });
      await adapter.recordOutcome({
        principleId: "p-1",
        handoffId: "2026-04-13-001",
        outcome: "helpful",
        recordedAt: "2026-04-13T01:00:00.000Z",
      });
      const all = await adapter.listOutcomes();
      expect(all).toHaveLength(2);
      expect(all[0]!.outcome).toBe("pending");
      expect(all[1]!.outcome).toBe("helpful");
    });

    it("skips malformed JSONL lines silently", async () => {
      await adapter.recordOutcome({
        principleId: "p-1",
        handoffId: "h1",
        outcome: "helpful",
        recordedAt: "2026-04-13T00:00:00.000Z",
      });
      const path = join(tempDir, ".maestro", "principles", "outcomes.jsonl");
      const existing = await readText(path) ?? "";
      await writeText(path, existing + "this is not json\n" + JSON.stringify({ invalid: true }) + "\n");

      const all = await adapter.listOutcomes();
      expect(all).toHaveLength(1);
      expect(all[0]!.principleId).toBe("p-1");
    });

    it("tail-caps at the given limit", async () => {
      for (let i = 0; i < 5; i++) {
        await adapter.recordOutcome({
          principleId: `p-${i}`,
          handoffId: `h-${i}`,
          outcome: "helpful",
          recordedAt: `2026-04-13T0${i}:00:00.000Z`,
        });
      }
      const last2 = await adapter.listOutcomes(2);
      expect(last2).toHaveLength(2);
      expect(last2[0]!.principleId).toBe("p-3");
      expect(last2[1]!.principleId).toBe("p-4");
    });

    it("refuses to record invalid outcome types (silent drop)", async () => {
      await adapter.recordOutcome({
        principleId: "p-1",
        handoffId: "h1",
        outcome: "not-an-outcome" as unknown as "helpful",
        recordedAt: "2026-04-13T00:00:00.000Z",
      });
      expect(await adapter.listOutcomes()).toEqual([]);
    });
  });

  describe("listPendingOutcomesForHandoff", () => {
    it("returns only rows where latest state is pending", async () => {
      await adapter.recordOutcome({
        principleId: "p-1",
        handoffId: "h1",
        outcome: "pending",
        recordedAt: "2026-04-13T00:00:00.000Z",
      });
      await adapter.recordOutcome({
        principleId: "p-2",
        handoffId: "h1",
        outcome: "pending",
        recordedAt: "2026-04-13T00:00:01.000Z",
      });
      // p-2 was later resolved
      await adapter.recordOutcome({
        principleId: "p-2",
        handoffId: "h1",
        outcome: "helpful",
        recordedAt: "2026-04-13T01:00:00.000Z",
      });
      // unrelated handoff
      await adapter.recordOutcome({
        principleId: "p-3",
        handoffId: "h2",
        outcome: "pending",
        recordedAt: "2026-04-13T02:00:00.000Z",
      });

      const pending = await adapter.listPendingOutcomesForHandoff("h1");
      expect(pending).toHaveLength(1);
      expect(pending[0]!.principleId).toBe("p-1");
    });
  });
});
