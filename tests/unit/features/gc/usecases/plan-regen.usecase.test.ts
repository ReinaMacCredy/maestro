import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { regenPlan } from "@/features/gc/usecases/plan-regen.usecase.js";
import type { TaskStorePort, Task } from "@/features/task";
import type { VerdictStorePort, Verdict } from "@/features/verdict";
import type { LegacySpecStorePort as SpecStorePort, Spec } from "@/shared/domain/legacy-spec";
import { FsEvidenceStoreAdapter, recordEvidence } from "@/features/evidence";

class FakeTaskStore implements Pick<TaskStorePort, "get" | "all"> {
  constructor(private map = new Map<string, Task>()) {}
  set(t: Task): void {
    this.map.set(t.id, t);
  }
  async get(id: string): Promise<Task | undefined> {
    return this.map.get(id);
  }
  async all(): Promise<readonly Task[]> {
    return [...this.map.values()];
  }
}

class FakeVerdictStore implements Pick<VerdictStorePort, "readLatest" | "history"> {
  constructor(private latest?: Verdict) {}
  async readLatest(): Promise<Verdict | undefined> {
    return this.latest;
  }
  async history(): Promise<readonly Verdict[]> {
    return this.latest ? [this.latest] : [];
  }
}

class FakeSpecStore implements Pick<SpecStorePort, "read"> {
  constructor(private spec?: Spec) {}
  async read(): Promise<Spec | undefined> {
    return this.spec;
  }
}

function tinyTask(id: string, missionId?: string): Task {
  return {
    id,
    title: id,
    status: "pending",
    type: "task",
    priority: 2,
    blockedBy: [],
    blocks: [],
    labels: [],
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...(missionId ? { missionId } : {}),
  } as unknown as Task;
}

let dir: string;

beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), "plan-regen-"));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

describe("regenPlan", () => {
  it("reports no-plan-file when no plan exists", async () => {
    const result = await regenPlan(
      {
        taskStore: new FakeTaskStore() as unknown as TaskStorePort,
        verdictStore: new FakeVerdictStore() as unknown as VerdictStorePort,
        specStore: new FakeSpecStore() as unknown as SpecStorePort,
        evidenceStore: new FsEvidenceStoreAdapter(dir),
      },
      { projectRoot: dir, taskId: "tsk-aaa111" },
    );
    expect(result.hasPlanFile).toBe(false);
    expect(result.drifts.some((d) => d.kind === "no-plan-file")).toBe(true);
  });

  it("reports stale-since-last-pass when evidence post-dates the latest PASS verdict", async () => {
    const ts = new FakeTaskStore();
    ts.set(tinyTask("tsk-aaa111"));
    const verdict: Verdict = {
      schemaVersion: 1,
      id: "vrd-1",
      taskId: "tsk-aaa111",
      contractVersion: 1,
      computedAt: "2026-01-01T00:00:00Z",
      decision: "PASS",
      effectiveRiskClass: "low",
      reasons: [],
      evidenceConsulted: [],
      policiesConsulted: [],
      trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
    };
    const evidenceStore = new FsEvidenceStoreAdapter(dir);
    await recordEvidence(evidenceStore, {
      task_id: "tsk-aaa111",
      kind: "manual-note",
      witness_level: "agent-claimed-and-not-reproducible",
      payload: { note: "after-pass" },
    });

    const result = await regenPlan(
      {
        taskStore: ts as unknown as TaskStorePort,
        verdictStore: new FakeVerdictStore(verdict) as unknown as VerdictStorePort,
        specStore: new FakeSpecStore() as unknown as SpecStorePort,
        evidenceStore,
      },
      { projectRoot: dir, taskId: "tsk-aaa111" },
    );
    expect(result.drifts.some((d) => d.kind === "stale-since-last-pass")).toBe(true);
  });

  it("reports missing-acceptance-coverage when plan does not cover spec criteria", async () => {
    const ts = new FakeTaskStore();
    ts.set(tinyTask("tsk-aaa111", "mis-1"));
    await mkdir(join(dir, ".maestro/plans"), { recursive: true });
    await writeFile(join(dir, ".maestro/plans/tsk-aaa111.md"), "# Plan\n\nUnrelated content\n");

    const spec = {
      id: "mis-1",
      schemaVersion: 1,
      missionId: "mis-1",
      acceptance_criteria: [
        { id: "ac-1", text: "Implement feature X with thorough error handling" },
      ],
      non_goals: [],
      runtime_signals: [],
      rollout_plan: { canary_stages: [], rollback_command: "" },
      runbook: "",
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    } as unknown as Spec;

    const result = await regenPlan(
      {
        taskStore: ts as unknown as TaskStorePort,
        verdictStore: new FakeVerdictStore() as unknown as VerdictStorePort,
        specStore: new FakeSpecStore(spec) as unknown as SpecStorePort,
        evidenceStore: new FsEvidenceStoreAdapter(dir),
      },
      { projectRoot: dir, taskId: "tsk-aaa111" },
    );
    expect(result.hasSpec).toBe(true);
    expect(result.drifts.some((d) => d.kind === "missing-acceptance-coverage")).toBe(true);
  });
});
