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
    subject: overrides.subject,
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

  describe("readLatestWithCorruption", () => {
    // Regression: FIX-12 -- the adapter swallows JSON.parse errors inside
    // scanTaskVerdicts (each malformed file resolves to undefined). The
    // corruption count is only observable through this method; if a future
    // simplification drops the corrupt counter, this test fails on real disk.
    it("returns corruptCount=1 when one verdict file is malformed JSON", async () => {
      const real = makeVerdict({ taskId: "tsk-aaaaaa" });
      await store.write("tsk-aaaaaa", real);
      const taskDir = join(tmpDir, ".maestro", "verdicts", "tsk-aaaaaa");
      await Bun.write(join(taskDir, `${generateVerdictId()}.json`), "{not valid json");

      const result = await store.readLatestWithCorruption("tsk-aaaaaa");
      expect(result.verdict?.id).toBe(real.id);
      expect(result.corruptCount).toBe(1);
    });

    it("counts multiple malformed verdict files independently", async () => {
      const real = makeVerdict({ taskId: "tsk-aaaaaa" });
      await store.write("tsk-aaaaaa", real);
      const taskDir = join(tmpDir, ".maestro", "verdicts", "tsk-aaaaaa");
      await Bun.write(join(taskDir, `${generateVerdictId()}.json`), "{bad 1");
      await Bun.write(join(taskDir, `${generateVerdictId()}.json`), "{bad 2");
      await Bun.write(join(taskDir, `${generateVerdictId()}.json`), "{bad 3");

      const result = await store.readLatestWithCorruption("tsk-aaaaaa");
      expect(result.verdict?.id).toBe(real.id);
      expect(result.corruptCount).toBe(3);
    });

    it("returns corruptCount=0 when all verdict files are valid", async () => {
      const v = makeVerdict({ taskId: "tsk-aaaaaa" });
      await store.write("tsk-aaaaaa", v);
      const result = await store.readLatestWithCorruption("tsk-aaaaaa");
      expect(result.corruptCount).toBe(0);
    });

    it("returns verdict=undefined, corruptCount=0 when the task has no verdicts at all", async () => {
      const result = await store.readLatestWithCorruption("tsk-aaaaaa");
      expect(result.verdict).toBeUndefined();
      expect(result.corruptCount).toBe(0);
    });

    it("non-json stray files don't inflate the corrupt count", async () => {
      const real = makeVerdict({ taskId: "tsk-aaaaaa" });
      await store.write("tsk-aaaaaa", real);
      const taskDir = join(tmpDir, ".maestro", "verdicts", "tsk-aaaaaa");
      await Bun.write(join(taskDir, "README.md"), "not a verdict");
      await Bun.write(join(taskDir, "stray.txt"), "garbage");

      const result = await store.readLatestWithCorruption("tsk-aaaaaa");
      expect(result.verdict?.id).toBe(real.id);
      expect(result.corruptCount).toBe(0);
    });
  });

  describe("findByTreeSha", () => {
    it("returns an empty array when no verdicts directory exists", async () => {
      const result = await store.findByTreeSha("abc123");
      expect(result).toEqual([]);
    });

    it("returns verdicts whose subject.tree_sha matches", async () => {
      const treeSha = "aabbccdd1234567890aabbccdd1234567890aabb";
      const match = makeVerdict({
        taskId: "tsk-aaaaaa",
        subject: { tree_sha: treeSha, pr: 1 },
      });
      const noMatch = makeVerdict({
        taskId: "tsk-aaaaaa",
        subject: { tree_sha: "0000000000000000000000000000000000000000" },
      });
      await store.write("tsk-aaaaaa", match);
      await store.write("tsk-aaaaaa", noMatch);

      const result = await store.findByTreeSha(treeSha);
      expect(result.map((v) => v.id)).toEqual([match.id]);
    });

    it("returns verdicts across multiple tasks with matching tree SHA", async () => {
      const treeSha = "1111111111111111111111111111111111111111";
      const v1 = makeVerdict({ taskId: "tsk-aaaaaa", subject: { tree_sha: treeSha } });
      const v2 = makeVerdict({ taskId: "tsk-bbbbbb", subject: { tree_sha: treeSha } });
      await store.write("tsk-aaaaaa", v1);
      await store.write("tsk-bbbbbb", v2);

      const result = await store.findByTreeSha(treeSha);
      const ids = result.map((v) => v.id).sort();
      expect(ids).toContain(v1.id);
      expect(ids).toContain(v2.id);
    });

    it("returns empty for verdicts without a subject field (v1 legacy rows)", async () => {
      const legacy = makeVerdict({ taskId: "tsk-aaaaaa" }); // no subject
      await store.write("tsk-aaaaaa", legacy);

      const result = await store.findByTreeSha("anytreeshavalue");
      expect(result).toEqual([]);
    });
  });
});
