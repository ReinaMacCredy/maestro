import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsVerdictStoreAdapter } from "@/features/verdict/adapters/fs-verdict-store.adapter.js";
import { generateVerdictId } from "@/features/verdict/domain/verdict-id.js";
import type { Verdict } from "@/features/verdict/domain/types.js";

function makeVerdict(overrides: Partial<Verdict> = {}): Verdict {
  return {
    schemaVersion: 1,
    id: overrides.id ?? generateVerdictId(),
    taskId: overrides.taskId ?? "tsk-aaaaaa",
    contractVersion: overrides.contractVersion ?? 1,
    computedAt: overrides.computedAt ?? "2026-05-04T10:00:00.000Z",
    decision: overrides.decision ?? "PASS",
    effectiveRiskClass: overrides.effectiveRiskClass ?? "medium",
    proposedRiskClass: overrides.proposedRiskClass,
    reasons: overrides.reasons ?? [{ category: "policy", code: "all-checks-passed", message: "All checks passed." }],
    evidenceConsulted: overrides.evidenceConsulted ?? [],
    policiesConsulted: overrides.policiesConsulted ?? [
      { file: "policies/risk.yaml", version: "1" },
    ],
    trustVerifier: overrides.trustVerifier ?? {
      findingsCount: 0,
      errors: 0,
      warns: 0,
      infos: 0,
    },
  };
}

describe("FsVerdictStoreAdapter", () => {
  let tmpDir: string;
  let store: FsVerdictStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "verdict-"));
    store = new FsVerdictStoreAdapter(tmpDir);
  });

  describe("write + readVersion round-trip", () => {
    it("writes a verdict and reads it back by version ID", async () => {
      const verdict = makeVerdict();
      await store.write(verdict.taskId, verdict);
      const result = await store.readVersion(verdict.taskId, verdict.id);
      expect(result).toEqual(verdict);
    });

    it("returns undefined for an unknown verdict ID", async () => {
      const verdict = makeVerdict();
      await store.write(verdict.taskId, verdict);
      const missing = generateVerdictId();
      const result = await store.readVersion(verdict.taskId, missing);
      expect(result).toBeUndefined();
    });
  });

  describe("readLatest", () => {
    it("returns undefined when no verdicts exist for the task", async () => {
      const result = await store.readLatest("tsk-aaaaaa");
      expect(result).toBeUndefined();
    });

    it("returns the verdict with the highest computedAt when multiple verdicts exist", async () => {
      const early = makeVerdict({ taskId: "tsk-aaaaaa", computedAt: "2026-05-04T08:00:00.000Z" });
      const middle = makeVerdict({ taskId: "tsk-aaaaaa", computedAt: "2026-05-04T09:00:00.000Z" });
      const latest = makeVerdict({ taskId: "tsk-aaaaaa", computedAt: "2026-05-04T10:00:00.000Z" });

      // Write in non-chronological order
      await store.write("tsk-aaaaaa", middle);
      await store.write("tsk-aaaaaa", latest);
      await store.write("tsk-aaaaaa", early);

      const result = await store.readLatest("tsk-aaaaaa");
      expect(result?.id).toBe(latest.id);
    });

    it("returns the single verdict when only one exists", async () => {
      const verdict = makeVerdict({ taskId: "tsk-bbbbbb" });
      await store.write("tsk-bbbbbb", verdict);
      const result = await store.readLatest("tsk-bbbbbb");
      expect(result?.id).toBe(verdict.id);
    });
  });

  describe("history", () => {
    it("returns an empty array when no verdicts exist", async () => {
      const result = await store.history("tsk-aaaaaa");
      expect(result).toEqual([]);
    });

    it("returns all verdicts in chronological order by computedAt", async () => {
      const early = makeVerdict({ taskId: "tsk-aaaaaa", computedAt: "2026-05-04T08:00:00.000Z" });
      const middle = makeVerdict({ taskId: "tsk-aaaaaa", computedAt: "2026-05-04T09:00:00.000Z" });
      const latest = makeVerdict({ taskId: "tsk-aaaaaa", computedAt: "2026-05-04T10:00:00.000Z" });

      await store.write("tsk-aaaaaa", latest);
      await store.write("tsk-aaaaaa", early);
      await store.write("tsk-aaaaaa", middle);

      const result = await store.history("tsk-aaaaaa");
      expect(result.map((v) => v.id)).toEqual([early.id, middle.id, latest.id]);
    });

    it("only returns verdicts for the requested task", async () => {
      const taskA = makeVerdict({ taskId: "tsk-aaaaaa" });
      const taskB = makeVerdict({ taskId: "tsk-bbbbbb" });

      await store.write("tsk-aaaaaa", taskA);
      await store.write("tsk-bbbbbb", taskB);

      const resultA = await store.history("tsk-aaaaaa");
      expect(resultA.map((v) => v.id)).toEqual([taskA.id]);

      const resultB = await store.history("tsk-bbbbbb");
      expect(resultB.map((v) => v.id)).toEqual([taskB.id]);
    });
  });

  describe("path safety", () => {
    it("rejects an invalid task_id on write", async () => {
      const verdict = makeVerdict({ taskId: "../etc/passwd" as string });
      await expect(store.write("../etc/passwd", verdict)).rejects.toThrow(/Invalid task ID/);
    });

    it("rejects a malformed verdict id on readVersion", async () => {
      await expect(store.readVersion("tsk-aaaaaa", "not-a-verdict-id")).rejects.toThrow();
    });
  });

  describe("tolerance for stray files", () => {
    it("silently skips non-json files and malformed JSON", async () => {
      const real = makeVerdict({ taskId: "tsk-aaaaaa" });
      await store.write("tsk-aaaaaa", real);
      const taskDir = join(tmpDir, ".maestro", "verdicts", "tsk-aaaaaa");
      await Bun.write(join(taskDir, "stray.txt"), "garbage");
      await Bun.write(join(taskDir, `${generateVerdictId()}.json`), "{bad json\n");

      const list = await store.history("tsk-aaaaaa");
      expect(list.map((v) => v.id)).toEqual([real.id]);
    });
  });
});
