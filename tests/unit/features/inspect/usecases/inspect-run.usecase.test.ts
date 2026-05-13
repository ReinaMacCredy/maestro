import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsEvidenceStoreAdapter, recordEvidence } from "@/features/evidence";
import { inspectRun, formatInspectRunLines } from "@/features/inspect";
import type { Verdict, VerdictStorePort } from "@/features/verdict";

class FakeVerdictStore implements VerdictStorePort {
  private rows: Verdict[] = [];
  push(v: Verdict): void {
    this.rows.push(v);
  }
  async write(): Promise<void> {
    /* noop */
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
  async findByTreeSha(): Promise<readonly Verdict[]> {
    return [];
  }
}

let dir: string;

beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), "inspect-"));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

function passVerdict(taskId: string, computedAt: string, id = "vrd-1"): Verdict {
  return {
    schemaVersion: 1,
    id,
    taskId,
    contractVersion: 1,
    computedAt,
    decision: "PASS",
    effectiveRiskClass: "low",
    reasons: [],
    evidenceConsulted: [],
    policiesConsulted: [],
    trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
  };
}

describe("inspectRun", () => {
  it("returns runDirExists=false when no run dir exists", async () => {
    const r = await inspectRun(
      {
        evidenceStore: new FsEvidenceStoreAdapter(dir),
        verdictStore: new FakeVerdictStore(),
      },
      { projectRoot: dir, taskId: "tsk-aaa111" },
    );
    expect(r.runDirExists).toBe(false);
    expect(r.artifacts.length).toBe(0);
  });

  it("collects orient.md and progress.md artifacts when present", async () => {
    const runDir = join(dir, ".maestro/runs/tsk-aaa111");
    await mkdir(runDir, { recursive: true });
    await writeFile(join(runDir, "orient.md"), "# Orient\nTask details\n");
    await writeFile(join(runDir, "progress.md"), "# Progress\nDone\n");
    const r = await inspectRun(
      {
        evidenceStore: new FsEvidenceStoreAdapter(dir),
        verdictStore: new FakeVerdictStore(),
      },
      { projectRoot: dir, taskId: "tsk-aaa111" },
    );
    expect(r.runDirExists).toBe(true);
    expect(r.artifacts.map((a) => a.file)).toEqual(["orient.md", "progress.md"]);
    expect(r.artifacts[0]?.excerpt).toContain("Orient");
  });

  it("returns recent evidence and verdicts limited by --tail", async () => {
    const evidenceStore = new FsEvidenceStoreAdapter(dir);
    for (let i = 0; i < 12; i++) {
      await recordEvidence(evidenceStore, {
        task_id: "tsk-aaa111",
        kind: "manual-note",
        witness_level: "agent-claimed-and-not-reproducible",
        payload: { note: `n${i}` },
      });
    }
    const verdicts = new FakeVerdictStore();
    verdicts.push(passVerdict("tsk-aaa111", "2026-05-01T00:00:00Z", "v-1"));
    verdicts.push(passVerdict("tsk-aaa111", "2026-05-02T00:00:00Z", "v-2"));

    const r = await inspectRun(
      { evidenceStore, verdictStore: verdicts },
      { projectRoot: dir, taskId: "tsk-aaa111", tail: 5 },
    );
    expect(r.evidence.length).toBe(5);
    expect(r.verdicts.length).toBe(2);
  });

  it("formats output with all sections", async () => {
    const r = await inspectRun(
      {
        evidenceStore: new FsEvidenceStoreAdapter(dir),
        verdictStore: new FakeVerdictStore(),
      },
      { projectRoot: dir, taskId: "tsk-aaa111" },
    );
    const lines = formatInspectRunLines(r);
    expect(lines.some((l) => l.startsWith("Inspecting run"))).toBe(true);
    expect(lines.some((l) => l.includes("Run dir:"))).toBe(true);
    expect(lines.some((l) => l.includes("Recent evidence"))).toBe(true);
    expect(lines.some((l) => l.includes("Verdict history"))).toBe(true);
  });
});
