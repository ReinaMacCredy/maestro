import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsEvidenceStoreAdapter } from "@/features/evidence";
import { ralphReview, type RalphFinding } from "@/features/ralph";

interface Fixtures {
  tmpDir: string;
  evidenceStore: FsEvidenceStoreAdapter;
}

let f: Fixtures;

beforeEach(async () => {
  const tmpDir = await mkdtemp(join(tmpdir(), "ralph-"));
  await mkdir(join(tmpDir, "src"), { recursive: true });
  f = { tmpDir, evidenceStore: new FsEvidenceStoreAdapter(tmpDir) };
});

describe("ralphReview", () => {
  it("converged=true when there are no error-severity findings", async () => {
    const result = await ralphReview(
      { evidenceStore: f.evidenceStore, previousIterations: async () => [] },
      { taskId: "tsk-aaa111", projectRoot: f.tmpDir },
    );
    expect(result.converged).toBe(true);
    expect(result.iteration).toBe(1);
    expect(result.stuck).toBe(false);
  });

  it("converged=false when an architecture lint fails at error severity", async () => {
    await writeFile(
      join(f.tmpDir, "src", "bad.ts"),
      'export function x() { Bun.spawn(["claude", "--help"]); }\n',
    );
    const result = await ralphReview(
      { evidenceStore: f.evidenceStore, previousIterations: async () => [] },
      { taskId: "tsk-bbb222", projectRoot: f.tmpDir },
    );
    expect(result.converged).toBe(false);
    expect(result.findings.some((x) => x.check === "no-runner-inversion")).toBe(true);
  });

  it("findingsHash is stable for the same set of findings", async () => {
    await writeFile(
      join(f.tmpDir, "src", "bad.ts"),
      'export function x() { Bun.spawn(["claude", "--help"]); }\n',
    );
    const a = await ralphReview(
      { evidenceStore: f.evidenceStore, previousIterations: async () => [] },
      { taskId: "tsk-ccc333", projectRoot: f.tmpDir },
    );
    const b = await ralphReview(
      { evidenceStore: f.evidenceStore, previousIterations: async () => [] },
      { taskId: "tsk-ccc333", projectRoot: f.tmpDir },
    );
    expect(a.findingsHash).toBe(b.findingsHash);
  });

  it("stuck=true when threshold-1 prior iterations share the hash", async () => {
    await writeFile(
      join(f.tmpDir, "src", "bad.ts"),
      'export function x() { Bun.spawn(["claude", "--help"]); }\n',
    );
    const sameHashIterations = [
      { iteration: 1, findingsHash: "" },
      { iteration: 2, findingsHash: "" },
    ];
    const probe = await ralphReview(
      { evidenceStore: f.evidenceStore, previousIterations: async () => [] },
      { taskId: "tsk-ddd444", projectRoot: f.tmpDir },
    );
    sameHashIterations[0]!.findingsHash = probe.findingsHash;
    sameHashIterations[1]!.findingsHash = probe.findingsHash;

    const result = await ralphReview(
      {
        evidenceStore: f.evidenceStore,
        previousIterations: async () => sameHashIterations,
      },
      { taskId: "tsk-ddd444", projectRoot: f.tmpDir, stuckThreshold: 3 },
    );
    expect(result.stuck).toBe(true);
    expect(result.iteration).toBe(3);
  });

  it("records ralph-iteration evidence at witnessed-by-maestro", async () => {
    const result = await ralphReview(
      { evidenceStore: f.evidenceStore, previousIterations: async () => [] },
      { taskId: "tsk-eee555", projectRoot: f.tmpDir },
    );
    expect(result.evidenceId).toMatch(/^evd-/);
    const rows = await f.evidenceStore.list({ task_id: "tsk-eee555", kind: "ralph-iteration" });
    expect(rows.length).toBe(1);
    expect(rows[0]!.witness_level).toBe("witnessed-by-maestro");
  });

  it("merges injected ai-review and threat-model findings", async () => {
    const aiFindings: RalphFinding[] = [
      { source: "ai-review", check: "security/x", severity: "warn", message: "watch this" },
    ];
    const tmFindings: RalphFinding[] = [
      { source: "threat-model", check: "missing", severity: "error", message: "model required" },
    ];
    const result = await ralphReview(
      {
        evidenceStore: f.evidenceStore,
        previousIterations: async () => [],
        listAiReviewFindings: async () => aiFindings,
        listThreatModelFindings: async () => tmFindings,
      },
      { taskId: "tsk-fff666", projectRoot: f.tmpDir },
    );
    expect(result.findings).toEqual(expect.arrayContaining([aiFindings[0]!, tmFindings[0]!]));
    expect(result.converged).toBe(false);
  });
});
