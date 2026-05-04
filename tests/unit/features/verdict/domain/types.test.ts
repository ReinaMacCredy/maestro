import { describe, expect, it } from "bun:test";
import type { Verdict, VerdictDecision } from "@/features/verdict/index.js";
import { FsVerdictStoreAdapter } from "@/features/verdict/adapters/fs-verdict-store.adapter.js";
import { mkdtemp, writeFile, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { generateVerdictId } from "@/features/verdict/domain/verdict-id.js";

describe("Verdict types", () => {
  it("accepts a fully-populated Verdict literal", () => {
    const v: Verdict = {
      schemaVersion: 1,
      id: "vrd-1714747200123-a1b2c3",
      taskId: "tsk-1714747200123-a1b2c3",
      contractVersion: 2,
      computedAt: "2026-05-04T00:00:00.000Z",
      decision: "PASS",
      proposedRiskClass: "low",
      effectiveRiskClass: "medium",
      reasons: [
        {
          category: "trust",
          code: "SCOPE_OK",
          message: "All files in scope",
          evidenceIds: ["evd-1714747200123-a1b2c3"],
          findingChecks: ["check-scope"],
          policyRuleIds: ["policy-1"],
        },
      ],
      evidenceConsulted: ["evd-1714747200123-a1b2c3"],
      policiesConsulted: [{ file: ".maestro/policies/owners.yaml", version: "1.0.0" }],
      trustVerifier: {
        findingsCount: 1,
        errors: 0,
        warns: 1,
        infos: 0,
      },
    };
    expect(v.schemaVersion).toBe(1);
    expect(v.decision).toBe("PASS");
  });

  it("accepts all four VerdictDecision values", () => {
    const decisions: VerdictDecision[] = ["PASS", "FAIL", "HUMAN", "BLOCK"];
    expect(decisions).toHaveLength(4);
  });

  it("rejects invalid VerdictDecision at type level", () => {
    // @ts-expect-error "UNKNOWN" is not a valid VerdictDecision
    const bad: VerdictDecision = "UNKNOWN";
    void bad;
  });

  it("accepts Verdict with minimal optional fields omitted", () => {
    const v: Verdict = {
      schemaVersion: 1,
      id: "vrd-1714747200123-ffffff",
      taskId: "tsk-1714747200123-ffffff",
      contractVersion: 1,
      computedAt: "2026-05-04T00:00:00.000Z",
      decision: "FAIL",
      effectiveRiskClass: "high",
      reasons: [],
      evidenceConsulted: [],
      policiesConsulted: [],
      trustVerifier: {
        findingsCount: 0,
        errors: 1,
        warns: 0,
        infos: 0,
      },
    };
    expect(v.proposedRiskClass).toBeUndefined();
    // subject is optional — backward compat
    expect(v.subject).toBeUndefined();
  });

  it("accepts Verdict with subject populated (tree SHA binding)", () => {
    const v: Verdict = {
      schemaVersion: 1,
      id: "vrd-1714747200123-aabbcc",
      taskId: "tsk-1714747200123-aabbcc",
      contractVersion: 1,
      computedAt: "2026-05-04T00:00:00.000Z",
      decision: "PASS",
      effectiveRiskClass: "low",
      reasons: [],
      evidenceConsulted: [],
      policiesConsulted: [],
      trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
      subject: { tree_sha: "abc123def456abc123def456abc123def456abc1", pr: 42 },
    };
    expect(v.subject?.tree_sha).toBe("abc123def456abc123def456abc123def456abc1");
    expect(v.subject?.pr).toBe(42);
  });

  it("v1 verdict without subject parses and round-trips via FsVerdictStoreAdapter", async () => {
    // Simulate a v1 verdict on disk written before subject was added.
    const tmpDir = await mkdtemp(join(tmpdir(), "verdict-backcompat-"));
    try {
      const verdictId = generateVerdictId();
      const taskId = "tsk-aaaaaa";
      const legacyVerdict = {
        schemaVersion: 1,
        id: verdictId,
        taskId,
        contractVersion: 1,
        computedAt: "2026-01-01T00:00:00.000Z",
        decision: "PASS",
        effectiveRiskClass: "medium",
        reasons: [],
        evidenceConsulted: [],
        policiesConsulted: [],
        trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
        // no subject field
      };
      const taskDir = join(tmpDir, ".maestro", "verdicts", taskId);
      await mkdir(taskDir, { recursive: true });
      await writeFile(join(taskDir, `${verdictId}.json`), JSON.stringify(legacyVerdict));

      const store = new FsVerdictStoreAdapter(tmpDir);
      const result = await store.readVersion(taskId, verdictId);
      expect(result).toBeDefined();
      expect(result?.id).toBe(verdictId);
      // subject is absent — backward compat preserved
      expect(result?.subject).toBeUndefined();
    } finally {
      const { rm } = await import("node:fs/promises");
      await rm(tmpDir, { recursive: true, force: true });
    }
  });
});
