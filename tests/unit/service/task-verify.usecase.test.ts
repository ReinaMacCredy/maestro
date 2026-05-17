import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type {
  ArchitectureRules,
  ArchitectureRulesPort,
} from "@/repo/architecture-rules.port.js";
import { ArchitectureRulesNotFoundError } from "@/repo/architecture-rules.port.js";
import type {
  EvidenceFilter,
  EvidenceRow,
  EvidenceStorePort,
} from "@/repo/evidence-store.port.js";
import {
  TaskNotFoundError,
  type CreateTaskInput,
  type TaskPatch,
  type TaskStorePort,
} from "@/repo/task-store.port.js";
import { TaskTransitionError, type TaskState } from "@/types/task-state.js";
import type { Task, TaskId } from "@/types/task.js";
import { taskVerify, TaskVerifyReasonRequiredError } from "@/service/task-verify.usecase.js";

const RULES: ArchitectureRules = {
  version: 1,
  forward_only: true,
  layers: ["types", "config", "repo", "service", "runtime", "ui"],
  cross_cutting: ["providers"],
  lint_scope: ["src/service/**/*.ts"],
  passive_harness: { forbidden_patterns: ["setInterval"] },
};

function stubRules(): ArchitectureRulesPort {
  return { load: async () => RULES };
}

function makeEvidence(): { store: EvidenceStorePort; rows: EvidenceRow[] } {
  const rows: EvidenceRow[] = [];
  return {
    rows,
    store: {
      async append(row) {
        rows.push(row);
      },
      async list(_filter?: EvidenceFilter) {
        return rows;
      },
      async read(id) {
        return rows.find((r) => r.id === id);
      },
    },
  };
}

function makeTaskStore(seed: readonly Task[] = []): TaskStorePort & { tasks: Map<TaskId, Task> } {
  const tasks = new Map<TaskId, Task>(seed.map((t) => [t.id, t]));
  return {
    tasks,
    async create(input: CreateTaskInput) {
      const now = new Date().toISOString();
      const task: Task = {
        id: `tsk-${tasks.size + 1}`,
        slug: input.slug,
        title: input.title,
        state: input.state,
        spec_path: input.spec_path,
        blocked_by: input.blocked_by ?? [],
        created_at: now,
        updated_at: now,
      };
      tasks.set(task.id, task);
      return task;
    },
    async createMany(inputs: readonly CreateTaskInput[]) {
      const out: Task[] = [];
      for (const i of inputs) out.push(await this.create(i));
      return out;
    },
    async get(id) {
      return tasks.get(id);
    },
    async update(id, patch: TaskPatch) {
      const existing = tasks.get(id);
      if (!existing) throw new TaskNotFoundError(id);
      const updated: Task = {
        ...existing,
        ...patch,
        id: existing.id,
        slug: existing.slug,
        created_at: existing.created_at,
        updated_at: new Date().toISOString(),
      };
      tasks.set(id, updated);
      return updated;
    },
    async list() {
      return [...tasks.values()];
    },
    async listByState(state: TaskState) {
      return [...tasks.values()].filter((t) => t.state === state);
    },
    async listByMissionId(mission_id: string) {
      return [...tasks.values()].filter((t) => t.mission_id === mission_id);
    },
  };
}

function seedTask(state: TaskState, id = "tsk-target"): Task {
  const now = new Date().toISOString();
  return {
    id,
    slug: "demo",
    title: "demo",
    state,
    blocked_by: [],
    created_at: now,
    updated_at: now,
  };
}

describe("taskVerify", () => {
  let repoRoot: string;

  beforeEach(async () => {
    repoRoot = await mkdtemp(join(tmpdir(), "maestro-task-verify-"));
    await mkdir(join(repoRoot, "src/service"), { recursive: true });
  });

  afterEach(async () => {
    await rm(repoRoot, { recursive: true, force: true });
  });

  it("PASS auto-advances claimed -> verifying -> ready and emits two transition rows", async () => {
    await writeFile(join(repoRoot, "src/service/clean.ts"), `export const X = 1;\n`);
    const taskStore = makeTaskStore([seedTask("claimed")]);
    const { store: evidenceStore, rows } = makeEvidence();

    const result = await taskVerify(
      { repoRoot, taskStore, evidenceStore, architectureRules: stubRules() },
      { id: "tsk-target" },
    );

    expect(result.verdict).toBe("PASS");
    expect(result.task.state).toBe("ready");
    expect(rows.length).toBe(2);
    expect(rows[0]).toMatchObject({
      kind: "transition",
      from_state: "claimed",
      to_state: "verifying",
      trigger_verb: "task:verify",
    });
    expect(rows[1]).toMatchObject({
      kind: "transition",
      from_state: "verifying",
      to_state: "ready",
      trigger_verb: "task:verify",
      verdict: "PASS",
    });
  });

  it("PASS from doing also auto-advances to ready", async () => {
    await writeFile(join(repoRoot, "src/service/clean.ts"), `export const X = 1;\n`);
    const taskStore = makeTaskStore([seedTask("doing")]);
    const { store: evidenceStore, rows } = makeEvidence();

    const result = await taskVerify(
      { repoRoot, taskStore, evidenceStore, architectureRules: stubRules() },
      { id: "tsk-target" },
    );

    expect(result.verdict).toBe("PASS");
    expect(result.task.state).toBe("ready");
    const enter = rows.find((r) => r.kind === "transition" && r.to_state === "verifying");
    expect(enter).toMatchObject({ from_state: "doing" });
  });

  it("PASS from verifying (re-run) does not re-emit the entry transition row", async () => {
    await writeFile(join(repoRoot, "src/service/clean.ts"), `export const X = 1;\n`);
    const taskStore = makeTaskStore([seedTask("verifying")]);
    const { store: evidenceStore, rows } = makeEvidence();

    const result = await taskVerify(
      { repoRoot, taskStore, evidenceStore, architectureRules: stubRules() },
      { id: "tsk-target" },
    );

    expect(result.verdict).toBe("PASS");
    expect(result.task.state).toBe("ready");
    expect(rows.length).toBe(1);
    expect(rows[0]).toMatchObject({
      kind: "transition",
      from_state: "verifying",
      to_state: "ready",
      verdict: "PASS",
    });
  });

  it("FAIL keeps state at verifying and emits one lint-violation row per finding with task_id set", async () => {
    await writeFile(
      join(repoRoot, "src/service/bad.ts"),
      `export function tick() { setInterval(() => null, 1000); setInterval(() => null, 2000); }\n`,
    );
    const taskStore = makeTaskStore([seedTask("claimed")]);
    const { store: evidenceStore, rows } = makeEvidence();

    const result = await taskVerify(
      { repoRoot, taskStore, evidenceStore, architectureRules: stubRules() },
      { id: "tsk-target" },
    );

    expect(result.verdict).toBe("FAIL");
    expect(result.task.state).toBe("verifying");
    expect(result.violations.length).toBe(2);

    const lintRows = rows.filter((r) => r.kind === "lint-violation");
    expect(lintRows.length).toBe(2);
    for (const row of lintRows) {
      expect(row).toMatchObject({
        kind: "lint-violation",
        task_id: "tsk-target",
        rule_id: "passive-harness",
        severity: "error",
      });
    }

    const transitions = rows.filter((r) => r.kind === "transition");
    expect(transitions.length).toBe(1);
    expect(transitions[0]).toMatchObject({
      kind: "transition",
      from_state: "claimed",
      to_state: "verifying",
    });
  });

  it("throws TaskNotFoundError when the id does not resolve", async () => {
    const taskStore = makeTaskStore();
    const { store: evidenceStore } = makeEvidence();
    await expect(
      taskVerify(
        { repoRoot, taskStore, evidenceStore, architectureRules: stubRules() },
        { id: "tsk-missing" },
      ),
    ).rejects.toBeInstanceOf(TaskNotFoundError);
  });

  it("throws TaskTransitionError when the source state is terminal (shipped)", async () => {
    const taskStore = makeTaskStore([seedTask("shipped")]);
    const { store: evidenceStore } = makeEvidence();
    await expect(
      taskVerify(
        { repoRoot, taskStore, evidenceStore, architectureRules: stubRules() },
        { id: "tsk-target" },
      ),
    ).rejects.toBeInstanceOf(TaskTransitionError);
  });

  it("re-enters verifying from ready (zero-diff PASS rescue path)", async () => {
    // Agent claimed, ran verify with no work done, got zero-violations PASS
    // and was advanced to ready. They then made the real edits and re-ran
    // verify. ready -> verifying must be allowed or the task is stranded.
    await writeFile(join(repoRoot, "src/service/clean.ts"), `export const X = 1;\n`);
    const taskStore = makeTaskStore([seedTask("ready")]);
    const { store: evidenceStore, rows } = makeEvidence();

    const result = await taskVerify(
      { repoRoot, taskStore, evidenceStore, architectureRules: stubRules() },
      { id: "tsk-target" },
    );

    expect(result.verdict).toBe("PASS");
    expect(result.task.state).toBe("ready");
    const entry = rows.find((r) => r.kind === "transition" && r.to_state === "verifying");
    expect(entry).toMatchObject({ from_state: "ready", to_state: "verifying" });
  });

  it("HUMAN verdict keeps task at verifying and emits a verdict=HUMAN evidence row with reason", async () => {
    const taskStore = makeTaskStore([seedTask("claimed")]);
    const { store: evidenceStore, rows } = makeEvidence();

    const result = await taskVerify(
      { repoRoot, taskStore, evidenceStore, architectureRules: stubRules() },
      { id: "tsk-target", verdict: "HUMAN", reason: "needs design review" },
    );

    expect(result.verdict).toBe("HUMAN");
    expect(result.task.state).toBe("verifying");
    expect(result.violations).toEqual([]);
    const verdictRow = rows.find(
      (r) => r.kind === "transition" && (r as { verdict?: string }).verdict === "HUMAN",
    );
    expect(verdictRow).toMatchObject({
      kind: "transition",
      from_state: "verifying",
      to_state: "verifying",
      trigger_verb: "task:verify",
      verdict: "HUMAN",
      reason: "needs design review",
    });
  });

  it("BLOCK verdict transitions verifying -> blocked and records block_reason + verdict=BLOCK row", async () => {
    const taskStore = makeTaskStore([seedTask("verifying")]);
    const { store: evidenceStore, rows } = makeEvidence();

    const result = await taskVerify(
      { repoRoot, taskStore, evidenceStore, architectureRules: stubRules() },
      { id: "tsk-target", verdict: "BLOCK", reason: "upstream API outage" },
    );

    expect(result.verdict).toBe("BLOCK");
    expect(result.task.state).toBe("blocked");
    expect(result.task.block_reason).toBe("upstream API outage");
    const verdictRow = rows.find(
      (r) => r.kind === "transition" && (r as { verdict?: string }).verdict === "BLOCK",
    );
    expect(verdictRow).toMatchObject({
      kind: "transition",
      from_state: "verifying",
      to_state: "blocked",
      trigger_verb: "task:verify",
      verdict: "BLOCK",
      reason: "upstream API outage",
    });
  });

  it("HUMAN verdict from claimed state still emits the entry transition (claimed -> verifying)", async () => {
    const taskStore = makeTaskStore([seedTask("claimed")]);
    const { store: evidenceStore, rows } = makeEvidence();

    await taskVerify(
      { repoRoot, taskStore, evidenceStore, architectureRules: stubRules() },
      { id: "tsk-target", verdict: "HUMAN", reason: "blocked on schema review" },
    );

    expect(rows.length).toBe(2);
    expect(rows[0]).toMatchObject({
      from_state: "claimed",
      to_state: "verifying",
      trigger_verb: "task:verify",
    });
    expect((rows[0] as { verdict?: string }).verdict).toBeUndefined();
    expect(rows[1]).toMatchObject({ verdict: "HUMAN" });
  });

  it("throws TaskVerifyReasonRequiredError when --verdict is set without a reason", async () => {
    const taskStore = makeTaskStore([seedTask("claimed")]);
    const { store: evidenceStore } = makeEvidence();

    await expect(
      taskVerify(
        { repoRoot, taskStore, evidenceStore, architectureRules: stubRules() },
        { id: "tsk-target", verdict: "HUMAN" },
      ),
    ).rejects.toBeInstanceOf(TaskVerifyReasonRequiredError);

    await expect(
      taskVerify(
        { repoRoot, taskStore, evidenceStore, architectureRules: stubRules() },
        { id: "tsk-target", verdict: "BLOCK", reason: "  " },
      ),
    ).rejects.toBeInstanceOf(TaskVerifyReasonRequiredError);
  });

  it("PASS when docs/architecture.yaml is absent (ArchitectureRulesNotFoundError treated as zero violations)", async () => {
    // External projects that lack docs/architecture.yaml should not crash on verify.
    const taskStore = makeTaskStore([seedTask("claimed")]);
    const { store: evidenceStore, rows } = makeEvidence();
    const missingRules: ArchitectureRulesPort = {
      load: async () => { throw new ArchitectureRulesNotFoundError("/no/such/path/docs/architecture.yaml"); },
    };

    const result = await taskVerify(
      { repoRoot, taskStore, evidenceStore, architectureRules: missingRules },
      { id: "tsk-target" },
    );

    expect(result.verdict).toBe("PASS");
    expect(result.task.state).toBe("ready");
    expect(result.violations).toEqual([]);
    const passRow = rows.find((r) => r.kind === "transition" && (r as { verdict?: string }).verdict === "PASS");
    expect(passRow).toBeDefined();
  });

  it("HUMAN verdict does not run lints (skips architecture rules)", async () => {
    // setInterval would normally trigger a violation under stubRules; HUMAN
    // verdict should skip lints entirely.
    await writeFile(
      join(repoRoot, "src/service/bad.ts"),
      `export function tick() { setInterval(() => null, 1000); }\n`,
    );
    const taskStore = makeTaskStore([seedTask("verifying")]);
    const { store: evidenceStore, rows } = makeEvidence();

    const result = await taskVerify(
      { repoRoot, taskStore, evidenceStore, architectureRules: stubRules() },
      { id: "tsk-target", verdict: "HUMAN", reason: "spec is ambiguous" },
    );

    expect(result.verdict).toBe("HUMAN");
    expect(result.violations).toEqual([]);
    const lints = rows.filter((r) => r.kind === "lint-violation");
    expect(lints.length).toBe(0);
  });
});
