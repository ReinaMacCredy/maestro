import { describe, expect, it } from "bun:test";
import type {
  CreateExecPlanInput,
  ExecPlanPatch,
  ExecPlanStorePort,
} from "@/v2/repo/exec-plan-store.port.js";
import type {
  EvidenceFilter,
  EvidenceRow,
  EvidenceStorePort,
} from "@/v2/repo/evidence-store.port.js";
import type {
  CreateTaskInput,
  TaskPatch,
  TaskStorePort,
} from "@/v2/repo/task-store.port.js";
import type { ExecPlan, ExecPlanId } from "@/v2/types/exec-plan.js";
import type { ExecPlanState } from "@/v2/types/exec-plan-state.js";
import type { Task, TaskId } from "@/v2/types/task.js";
import type { TaskState } from "@/v2/types/task-state.js";
import { tryAdvancePlan } from "@/v2/service/try-advance-plan.usecase.js";

const FROZEN = new Date("2026-05-15T11:00:00.000Z");

function makeStores(): {
  planStore: ExecPlanStorePort;
  plans: Map<ExecPlanId, ExecPlan>;
  taskStore: TaskStorePort;
  tasks: Map<TaskId, Task>;
  evidenceStore: EvidenceStorePort;
  evidence: EvidenceRow[];
} {
  const plans = new Map<ExecPlanId, ExecPlan>();
  const tasks = new Map<TaskId, Task>();
  const evidence: EvidenceRow[] = [];
  let planN = 0;
  let taskN = 0;
  const planStore: ExecPlanStorePort = {
    async create(input: CreateExecPlanInput) {
      planN += 1;
      const plan: ExecPlan = {
        id: `pln-${planN}`,
        slug: input.slug,
        title: input.title,
        state: input.state,
        spec_path: input.spec_path,
        created_at: FROZEN.toISOString(),
        updated_at: FROZEN.toISOString(),
      };
      plans.set(plan.id, plan);
      return plan;
    },
    async get(id) {
      return plans.get(id);
    },
    async update(id, patch: ExecPlanPatch) {
      const existing = plans.get(id);
      if (!existing) throw new Error("not found");
      const next: ExecPlan = { ...existing, ...patch, updated_at: FROZEN.toISOString() };
      plans.set(id, next);
      return next;
    },
    async list() {
      return [...plans.values()];
    },
    async listByState(state: ExecPlanState) {
      return [...plans.values()].filter((p) => p.state === state);
    },
  };
  const taskStore: TaskStorePort = {
    async create(input: CreateTaskInput) {
      taskN += 1;
      const task: Task = {
        id: `tsk-${taskN}`,
        slug: input.slug,
        title: input.title,
        state: input.state,
        spec_path: input.spec_path,
        plan_id: input.plan_id,
        blocked_by: input.blocked_by ?? [],
        created_at: FROZEN.toISOString(),
        updated_at: FROZEN.toISOString(),
      };
      tasks.set(task.id, task);
      return task;
    },
    async get(id) {
      return tasks.get(id);
    },
    async update(id, patch: TaskPatch) {
      const existing = tasks.get(id);
      if (!existing) throw new Error("not found");
      const next: Task = { ...existing, ...patch, updated_at: FROZEN.toISOString() };
      tasks.set(id, next);
      return next;
    },
    async list() {
      return [...tasks.values()];
    },
    async listByState(state: TaskState) {
      return [...tasks.values()].filter((t) => t.state === state);
    },
    async listByPlanId(plan_id: string) {
      return [...tasks.values()].filter((t) => t.plan_id === plan_id);
    },
  };
  const evidenceStore: EvidenceStorePort = {
    async append(row) {
      evidence.push(row);
    },
    async list(_filter?: EvidenceFilter) {
      return evidence;
    },
  };
  return { planStore, plans, taskStore, tasks, evidenceStore, evidence };
}

async function seedPlanWithTasks(
  ps: ExecPlanStorePort,
  ts: TaskStorePort,
  planState: ExecPlanState,
  taskStates: readonly TaskState[],
): Promise<ExecPlan> {
  const plan = await ps.create({ slug: "demo", title: "Demo", state: planState });
  for (let i = 0; i < taskStates.length; i += 1) {
    await ts.create({
      slug: `child-${i + 1}`,
      title: `Child ${i + 1}`,
      state: taskStates[i]!,
      plan_id: plan.id,
    });
  }
  return plan;
}

describe("tryAdvancePlan", () => {
  it("is a no-op when plan_id is undefined", async () => {
    const { planStore, taskStore, evidenceStore, evidence } = makeStores();
    const out = await tryAdvancePlan(
      { planStore, taskStore, evidenceStore },
      { plan_id: undefined, trigger_task_verb: "task:claim" },
    );
    expect(out).toBeUndefined();
    expect(evidence.length).toBe(0);
  });

  it("is a no-op when the plan id does not exist", async () => {
    const { planStore, taskStore, evidenceStore, evidence } = makeStores();
    const out = await tryAdvancePlan(
      { planStore, taskStore, evidenceStore },
      { plan_id: "pln-missing", trigger_task_verb: "task:claim" },
    );
    expect(out).toBeUndefined();
    expect(evidence.length).toBe(0);
  });

  it("advances planned -> in-progress on first task:claim and emits plan:auto-start", async () => {
    const { planStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(planStore, taskStore, "planned", [
      "claimed",
      "draft",
      "draft",
    ]);

    const out = await tryAdvancePlan(
      { planStore, taskStore, evidenceStore },
      { plan_id: plan.id, trigger_task_verb: "task:claim" },
    );

    expect(out!.state).toBe("in-progress");
    expect(evidence.length).toBe(1);
    expect(evidence[0]).toMatchObject({
      kind: "transition",
      plan_id: plan.id,
      from_state: "planned",
      to_state: "in-progress",
      trigger_verb: "plan:auto-start",
    });
  });

  it("is idempotent: a second task:claim against an in-progress plan emits nothing", async () => {
    const { planStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(planStore, taskStore, "in-progress", [
      "claimed",
      "doing",
      "draft",
    ]);

    const out = await tryAdvancePlan(
      { planStore, taskStore, evidenceStore },
      { plan_id: plan.id, trigger_task_verb: "task:claim" },
    );

    expect(out!.state).toBe("in-progress");
    expect(evidence.length).toBe(0);
  });

  it("does not auto-complete on task:ship while a sibling is still non-terminal", async () => {
    const { planStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(planStore, taskStore, "in-progress", [
      "shipped",
      "doing",
    ]);

    const out = await tryAdvancePlan(
      { planStore, taskStore, evidenceStore },
      { plan_id: plan.id, trigger_task_verb: "task:ship" },
    );

    expect(out!.state).toBe("in-progress");
    expect(evidence.length).toBe(0);
  });

  it("advances in-progress -> completed on task:ship when every child is terminal", async () => {
    const { planStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(planStore, taskStore, "in-progress", [
      "shipped",
      "shipped",
      "abandoned",
    ]);

    const out = await tryAdvancePlan(
      { planStore, taskStore, evidenceStore },
      { plan_id: plan.id, trigger_task_verb: "task:ship" },
    );

    expect(out!.state).toBe("completed");
    expect(evidence.length).toBe(1);
    expect(evidence[0]).toMatchObject({
      kind: "transition",
      plan_id: plan.id,
      from_state: "in-progress",
      to_state: "completed",
      trigger_verb: "plan:auto-complete",
    });
  });

  it("steps through in-progress when planned plan has all children terminal (abandoned-before-claim path)", async () => {
    const { planStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(planStore, taskStore, "planned", [
      "abandoned",
      "abandoned",
    ]);

    const out = await tryAdvancePlan(
      { planStore, taskStore, evidenceStore },
      { plan_id: plan.id, trigger_task_verb: "task:abandon" },
    );

    expect(out!.state).toBe("completed");
    expect(evidence.length).toBe(2);
    expect(evidence[0]).toMatchObject({
      from_state: "planned",
      to_state: "in-progress",
      trigger_verb: "plan:auto-start",
    });
    expect(evidence[1]).toMatchObject({
      from_state: "in-progress",
      to_state: "completed",
      trigger_verb: "plan:auto-complete",
    });
  });

  it("is a no-op for plans already at a terminal state", async () => {
    const { planStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(planStore, taskStore, "completed", [
      "shipped",
      "shipped",
    ]);

    const out = await tryAdvancePlan(
      { planStore, taskStore, evidenceStore },
      { plan_id: plan.id, trigger_task_verb: "task:ship" },
    );

    expect(out!.state).toBe("completed");
    expect(evidence.length).toBe(0);
  });

  it("is a no-op when called with task:claim against an in-progress plan (claim only matters for planned)", async () => {
    const { planStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(planStore, taskStore, "in-progress", [
      "claimed",
      "draft",
    ]);

    const out = await tryAdvancePlan(
      { planStore, taskStore, evidenceStore },
      { plan_id: plan.id, trigger_task_verb: "task:claim" },
    );

    expect(out!.state).toBe("in-progress");
    expect(evidence.length).toBe(0);
  });
});
