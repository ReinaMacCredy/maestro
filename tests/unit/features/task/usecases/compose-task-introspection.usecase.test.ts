import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsTaskContinuationHistoryStoreAdapter } from "@/features/task/adapters/fs-task-continuation-history-store.adapter.js";
import { FsTaskContinuationStoreAdapter } from "@/features/task/adapters/fs-task-continuation-store.adapter.js";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { FsContractStoreAdapter } from "@/features/task/adapters/fs-contract-store.adapter.js";
import { FsContractVersionStoreAdapter } from "@/features/task/adapters/fs-contract-version-store.adapter.js";
import { FsRunStateStoreAdapter } from "@/features/task/adapters/fs-run-state-store.adapter.js";
import { FsEvidenceStoreAdapter, recordEvidence } from "@/features/evidence";
import { FsSpecStoreAdapter } from "@/features/spec";
import type { Verdict, VerdictStorePort } from "@/features/verdict";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import {
  composeTaskIntrospection,
  formatTaskIntrospectionMarkdown,
} from "@/features/task/usecases/compose-task-introspection.usecase.js";

class StubVerdictStore implements VerdictStorePort {
  private store = new Map<string, Verdict>();
  setLatest(taskId: string, verdict: Verdict): void {
    this.store.set(taskId, verdict);
  }
  async write(taskId: string, verdict: Verdict): Promise<void> {
    this.store.set(taskId, verdict);
  }
  async readLatest(taskId: string): Promise<Verdict | undefined> {
    return this.store.get(taskId);
  }
  async readVersion(): Promise<Verdict | undefined> {
    return undefined;
  }
  async history(taskId: string): Promise<readonly Verdict[]> {
    const v = this.store.get(taskId);
    return v ? [v] : [];
  }
  async findByTreeSha(): Promise<readonly Verdict[]> {
    return [];
  }
}

describe("composeTaskIntrospection", () => {
  let tmpDir: string;
  let taskStore: JsonlTaskStoreAdapter;
  let continuationStore: FsTaskContinuationStoreAdapter;
  let continuationHistory: FsTaskContinuationHistoryStoreAdapter;
  let evidenceStore: FsEvidenceStoreAdapter;
  let specStore: FsSpecStoreAdapter;
  let verdictStore: StubVerdictStore;
  let runStateStore: FsRunStateStoreAdapter;
  let contractStore: FsContractStoreAdapter;
  let contractVersionStore: FsContractVersionStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-introspect-"));
    taskStore = new JsonlTaskStoreAdapter(tmpDir);
    continuationStore = new FsTaskContinuationStoreAdapter(tmpDir);
    continuationHistory = new FsTaskContinuationHistoryStoreAdapter(tmpDir);
    evidenceStore = new FsEvidenceStoreAdapter(tmpDir);
    specStore = new FsSpecStoreAdapter(tmpDir);
    verdictStore = new StubVerdictStore();
    runStateStore = new FsRunStateStoreAdapter(tmpDir);
    contractStore = new FsContractStoreAdapter(tmpDir);
    contractVersionStore = new FsContractVersionStoreAdapter(tmpDir);
  });

  function deps() {
    return {
      taskStore,
      continuationStore,
      continuationHistory,
      specStore,
      verdictStore,
      evidenceStore,
      runStateStore,
      contractStore,
      contractVersionStore,
      repoRoot: tmpDir,
      // Deterministic: never run real git in tests.
      resolveCommitsSince: async () => [],
    };
  }

  it("returns view with spec=undefined when task has no missionId", async () => {
    const task = await createTask(taskStore, { title: "No mission" });
    const view = await composeTaskIntrospection(deps(), task.id);
    expect(view.spec).toBeUndefined();
    expect(view.task.id).toBe(task.id);
  });

  it("returns view with spec=undefined when missionId is set but no spec written", async () => {
    const task = await createTask(taskStore, { title: "Mission no spec" });
    const view = await composeTaskIntrospection(deps(), task.id);
    expect(view.spec).toBeUndefined();
  });

  it("populates lastVerdict from verdictStore.readLatest", async () => {
    const task = await createTask(taskStore, { title: "With verdict" });
    const verdict: Verdict = {
      schemaVersion: 1,
      id: "vrd-1234567890123-abcdef",
      taskId: task.id,
      contractVersion: 1,
      computedAt: "2026-05-01T00:00:00.000Z",
      decision: "PASS",
      effectiveRiskClass: "low",
      reasons: [],
      evidenceConsulted: [],
      policiesConsulted: [],
      trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
    };
    verdictStore.setLatest(task.id, verdict);
    const view = await composeTaskIntrospection(deps(), task.id);
    expect(view.lastVerdict?.decision).toBe("PASS");
  });

  it("filters open lint-violation evidence into openLintViolations", async () => {
    const task = await createTask(taskStore, { title: "With lint" });
    await recordEvidence(evidenceStore, {
      task_id: task.id,
      kind: "lint-violation",
      witness_level: "agent-claimed-locally",
      payload: {
        ruleId: "no-runner-inversion",
        file: "src/foo.ts",
        line: 42,
        message: "Forbidden subprocess spawn",
        remediation: "Drop the spawn",
      },
    });
    await recordEvidence(evidenceStore, {
      task_id: task.id,
      kind: "manual-note",
      witness_level: "agent-claimed-locally",
      payload: { note: "unrelated" },
    });
    const view = await composeTaskIntrospection(deps(), task.id);
    expect(view.openLintViolations.length).toBe(1);
    expect(view.openLintViolations[0]!.kind).toBe("lint-violation");
  });

  it("returns empty recentCommits when no session-start exists", async () => {
    const task = await createTask(taskStore, { title: "No session" });
    const view = await composeTaskIntrospection(deps(), task.id);
    expect(view.recentCommits).toEqual([]);
    expect(view.sessionAnchorSha).toBeUndefined();
  });

  it("uses latest session-start headSha as commit anchor", async () => {
    const task = await createTask(taskStore, { title: "With sessions" });
    await recordEvidence(evidenceStore, {
      task_id: task.id,
      kind: "session-start",
      witness_level: "witnessed-by-maestro",
      payload: { taskId: task.id, headSha: "old-sha" },
    });
    // Bump created_at by 1ms via a small sleep. recordEvidence uses Date.now()
    // and an evidence-id timestamp segment, so consecutive calls naturally diverge.
    await new Promise((resolve) => setTimeout(resolve, 5));
    await recordEvidence(evidenceStore, {
      task_id: task.id,
      kind: "session-start",
      witness_level: "witnessed-by-maestro",
      payload: { taskId: task.id, headSha: "new-sha" },
    });
    const calls: { repoRoot: string; anchorSha: string }[] = [];
    const view = await composeTaskIntrospection(
      {
        ...deps(),
        checkCommitReachable: async () => true,
        resolveCommitsSince: async (repoRoot, anchorSha) => {
          calls.push({ repoRoot, anchorSha });
          return [{ sha: "abc1234", subject: "wip" }];
        },
      },
      task.id,
    );
    expect(view.sessionAnchorSha).toBe("new-sha");
    expect(calls).toEqual([{ repoRoot: tmpDir, anchorSha: "new-sha" }]);
    expect(view.recentCommits.length).toBe(1);
  });

  it("formatTaskIntrospectionMarkdown renders all 8 sections", async () => {
    const task = await createTask(taskStore, { title: "Render test" });
    const view = await composeTaskIntrospection(deps(), task.id);
    const md = formatTaskIntrospectionMarkdown(view);
    expect(md).toContain(`# Task: ${task.id}`);
    expect(md).toContain("## Spec — Acceptance Criteria");
    expect(md).toContain("## Spec — Non-goals");
    expect(md).toContain("## Plan position");
    expect(md).toContain("## Verdict");
    expect(md).toContain("## Budget");
    expect(md).toContain("## Open lints");
    expect(md).toContain("## Open blockers");
    expect(md).toContain("## Recent evidence");
    expect(md).toContain("## Recent commits");
  });

  it("formatTaskIntrospectionMarkdown emits 'No spec recorded' notice when missing", async () => {
    const task = await createTask(taskStore, { title: "No spec" });
    const view = await composeTaskIntrospection(deps(), task.id);
    const md = formatTaskIntrospectionMarkdown(view);
    expect(md).toContain("No spec recorded for this task.");
  });

  it("formatTaskIntrospectionMarkdown emits 'No verdict requested' notice when missing", async () => {
    const task = await createTask(taskStore, { title: "No verdict" });
    const view = await composeTaskIntrospection(deps(), task.id);
    const md = formatTaskIntrospectionMarkdown(view);
    expect(md).toContain("No verdict requested for current tree.");
  });

  it("formatTaskIntrospectionMarkdown lists open lint violations with location", async () => {
    const task = await createTask(taskStore, { title: "Lint render" });
    await recordEvidence(evidenceStore, {
      task_id: task.id,
      kind: "lint-violation",
      witness_level: "agent-claimed-locally",
      payload: {
        ruleId: "no-runner-inversion",
        file: "src/foo.ts",
        line: 42,
        message: "Forbidden subprocess spawn",
        remediation: "Drop the spawn",
      },
    });
    const view = await composeTaskIntrospection(deps(), task.id);
    const md = formatTaskIntrospectionMarkdown(view);
    expect(md).toContain("## Open lints (1)");
    expect(md).toContain("no-runner-inversion: src/foo.ts:42");
  });
});
