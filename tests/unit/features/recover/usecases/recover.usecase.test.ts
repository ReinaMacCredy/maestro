import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, writeFile, access, readdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsEvidenceStoreAdapter } from "@/features/evidence";
import type { Verdict, VerdictStorePort } from "@/features/verdict";
import { recoverTask } from "@/features/recover";

class FakeVerdictStore implements VerdictStorePort {
  private rows: Verdict[] = [];
  push(v: Verdict): void {
    this.rows.push(v);
  }
  async write(_taskId: string, v: Verdict): Promise<void> {
    this.rows.push(v);
  }
  async readLatest(taskId: string): Promise<Verdict | undefined> {
    return [...this.rows].reverse().find((v) => v.taskId === taskId);
  }
  async readVersion(): Promise<Verdict | undefined> {
    return undefined;
  }
  async history(taskId: string): Promise<readonly Verdict[]> {
    return this.rows.filter((v) => v.taskId === taskId);
  }
  async findByTreeSha(treeSha: string): Promise<readonly Verdict[]> {
    return this.rows.filter((v) => v.subject?.tree_sha === treeSha);
  }
  async readLatestWithCorruption(taskId: string): Promise<{ verdict: Verdict | undefined; corruptCount: number }> {
    return { verdict: await this.readLatest(taskId), corruptCount: 0 };
  }
}

function passVerdict(taskId: string, treeSha: string, id = "vrd-pass-1"): Verdict {
  return {
    schemaVersion: 1,
    id,
    taskId,
    subject: { tree_sha: treeSha },
    contractVersion: 1,
    computedAt: "2026-05-08T00:00:00Z",
    decision: "PASS",
    effectiveRiskClass: "low",
    reasons: [],
    evidenceConsulted: [],
    policiesConsulted: [],
    trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
  };
}

interface Fixtures {
  tmpDir: string;
  evidenceStore: FsEvidenceStoreAdapter;
  verdictStore: FakeVerdictStore;
  resetCalls: { commit: string }[];
  deps: () => Parameters<typeof recoverTask>[0];
}

let f: Fixtures;

beforeEach(async () => {
  const tmpDir = await mkdtemp(join(tmpdir(), "recover-"));
  await mkdir(join(tmpDir, ".maestro", "runs", "tsk-abc123"), { recursive: true });
  await writeFile(join(tmpDir, ".maestro", "runs", "tsk-abc123", "state.json"), "{}");
  const evidenceStore = new FsEvidenceStoreAdapter(tmpDir);
  const verdictStore = new FakeVerdictStore();
  const resetCalls: { commit: string }[] = [];
  f = {
    tmpDir,
    evidenceStore,
    verdictStore,
    resetCalls,
    deps: () => ({
      evidenceStore,
      verdictStore,
      resolveHeadCommit: async () => "headcommit0000",
      resolveCommitForTree: async (_cwd, treeSha) => `commit-for-${treeSha}`,
      resolveRef: async (_cwd, ref) => `commit-for-ref-${ref}`,
      checkDirtyTree: async () => false,
      resetHard: async (_cwd, commit) => {
        resetCalls.push({ commit });
      },
    }),
  };
});

describe("recoverTask", () => {
  it("resets to the last PASS verdict's tree by default", async () => {
    f.verdictStore.push(passVerdict("tsk-abc123", "tree-abc"));
    const result = await recoverTask(f.deps(), { taskId: "tsk-abc123", projectRoot: f.tmpDir });
    expect(result.applied).toBe(true);
    expect(result.plan.toCommit).toBe("commit-for-tree-abc");
    expect(result.plan.reason).toBe("verdict-anchored");
    expect(f.resetCalls).toEqual([{ commit: "commit-for-tree-abc" }]);
  });

  it("uses --to override when provided", async () => {
    f.verdictStore.push(passVerdict("tsk-abc123", "tree-abc"));
    const result = await recoverTask(f.deps(), {
      taskId: "tsk-abc123",
      projectRoot: f.tmpDir,
      to: "abc123",
    });
    expect(result.applied).toBe(true);
    expect(result.plan.toCommit).toBe("commit-for-ref-abc123");
    expect(result.plan.reason).toBe("explicit-ref");
  });

  it("throws when no PASS verdict exists and no --to provided", async () => {
    expect(recoverTask(f.deps(), { taskId: "tsk-abc123", projectRoot: f.tmpDir })).rejects.toThrow(
      /No PASS verdict/,
    );
  });

  it("refuses to reset when working tree is dirty without --force", async () => {
    f.verdictStore.push(passVerdict("tsk-abc123", "tree-abc"));
    const deps = { ...f.deps(), checkDirtyTree: async () => true };
    expect(recoverTask(deps, { taskId: "tsk-abc123", projectRoot: f.tmpDir })).rejects.toThrow(
      /uncommitted changes/,
    );
  });

  it("resets despite dirty tree when --force is set", async () => {
    f.verdictStore.push(passVerdict("tsk-abc123", "tree-abc"));
    const deps = { ...f.deps(), checkDirtyTree: async () => true };
    const result = await recoverTask(deps, {
      taskId: "tsk-abc123",
      projectRoot: f.tmpDir,
      force: true,
    });
    expect(result.applied).toBe(true);
  });

  it("dry-run does not call resetHard or record evidence", async () => {
    f.verdictStore.push(passVerdict("tsk-abc123", "tree-abc"));
    const result = await recoverTask(f.deps(), {
      taskId: "tsk-abc123",
      projectRoot: f.tmpDir,
      dryRun: true,
    });
    expect(result.applied).toBe(false);
    expect(f.resetCalls).toEqual([]);
    const dir = await readdir(join(f.tmpDir, ".maestro", "evidence", "tsk-abc123")).catch(() => []);
    expect(dir.length).toBe(0);
  });

  it("records kind=recovery evidence at witnessed-by-maestro", async () => {
    f.verdictStore.push(passVerdict("tsk-abc123", "tree-abc", "vrd-x"));
    const result = await recoverTask(f.deps(), { taskId: "tsk-abc123", projectRoot: f.tmpDir });
    expect(result.evidenceId).toMatch(/^evd-/);
    const rows = await f.evidenceStore.list({ task_id: "tsk-abc123", kind: "recovery" });
    expect(rows.length).toBe(1);
    expect(rows[0]!.witness_level).toBe("witnessed-by-maestro");
    const payload = rows[0]!.payload as {
      fromCommit: string;
      toCommit: string;
      anchorVerdictId?: string;
      reason: string;
    };
    expect(payload.fromCommit).toBe("headcommit0000");
    expect(payload.toCommit).toBe("commit-for-tree-abc");
    expect(payload.anchorVerdictId).toBe("vrd-x");
    expect(payload.reason).toBe("verdict-anchored");
  });

  it("drops .maestro/runs/<taskId>/ on apply", async () => {
    f.verdictStore.push(passVerdict("tsk-abc123", "tree-abc"));
    await recoverTask(f.deps(), { taskId: "tsk-abc123", projectRoot: f.tmpDir });
    expect(access(join(f.tmpDir, ".maestro", "runs", "tsk-abc123"))).rejects.toThrow();
  });

  it("returns applied=false when fromCommit equals toCommit", async () => {
    f.verdictStore.push(passVerdict("tsk-abc123", "tree-abc"));
    const deps = {
      ...f.deps(),
      resolveCommitForTree: async () => "headcommit0000",
    };
    const result = await recoverTask(deps, { taskId: "tsk-abc123", projectRoot: f.tmpDir });
    expect(result.applied).toBe(false);
    expect(f.resetCalls).toEqual([]);
  });
});
