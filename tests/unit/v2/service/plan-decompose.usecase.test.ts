import { describe, expect, it } from "bun:test";
import type {
  CreateExecPlanInput,
  ExecPlanPatch,
  ExecPlanStorePort,
} from "@/v2/repo/exec-plan-store.port.js";
import { ExecPlanNotFoundError } from "@/v2/repo/exec-plan-store.port.js";
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
import { DuplicateSlugError } from "@/v2/repo/task-store.port.js";
import type { ExecPlan, ExecPlanId } from "@/v2/types/exec-plan.js";
import type { ExecPlanState } from "@/v2/types/exec-plan-state.js";
import { ExecPlanTransitionError } from "@/v2/types/exec-plan-state.js";
import type { Task, TaskId } from "@/v2/types/task.js";
import type { TaskState } from "@/v2/types/task-state.js";
import {
  parsePlanDecomposeBatch,
  planDecompose,
  PlanDecomposeBatchEmptyError,
  PlanDecomposeBatchInvalidError,
  PlanDecomposeDuplicateSlugInBatchError,
} from "@/v2/service/plan-decompose.usecase.js";

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
      if ([...tasks.values()].some((t) => t.slug === input.slug)) {
        throw new DuplicateSlugError(input.slug);
      }
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

async function seedSpecifiedPlan(planStore: ExecPlanStorePort): Promise<ExecPlan> {
  return planStore.create({
    slug: "demo-plan",
    title: "Demo plan",
    state: "specified",
  });
}

describe("parsePlanDecomposeBatch", () => {
  it("accepts a JSON array of task objects", () => {
    const out = parsePlanDecomposeBatch([
      { title: "A", slug: "a" },
      { title: "B", slug: "b", spec_path: "specs/b.md" },
    ]);
    expect(out.length).toBe(2);
    expect(out[0]).toEqual({ title: "A", slug: "a", spec_path: undefined });
    expect(out[1]).toEqual({ title: "B", slug: "b", spec_path: "specs/b.md" });
  });

  it("accepts a wrapper object with a 'tasks' array", () => {
    const out = parsePlanDecomposeBatch({ tasks: [{ title: "A", slug: "a" }] });
    expect(out.length).toBe(1);
  });

  it("rejects an empty batch", () => {
    expect(() => parsePlanDecomposeBatch([])).toThrow(PlanDecomposeBatchEmptyError);
  });

  it("rejects a non-array, non-object root", () => {
    expect(() => parsePlanDecomposeBatch("nope")).toThrow(PlanDecomposeBatchInvalidError);
  });

  it("rejects a task missing a title", () => {
    expect(() => parsePlanDecomposeBatch([{ slug: "a" }])).toThrow(PlanDecomposeBatchInvalidError);
  });

  it("rejects a task missing a slug", () => {
    expect(() => parsePlanDecomposeBatch([{ title: "A" }])).toThrow(
      PlanDecomposeBatchInvalidError,
    );
  });

  it("rejects spec_path that is not a non-empty string", () => {
    expect(() => parsePlanDecomposeBatch([{ title: "A", slug: "a", spec_path: 42 }])).toThrow(
      PlanDecomposeBatchInvalidError,
    );
  });
});

describe("planDecompose", () => {
  it("creates child tasks linked by plan_id and advances the plan to 'planned'", async () => {
    const { planStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedSpecifiedPlan(planStore);

    const result = await planDecompose(
      { planStore, taskStore, evidenceStore },
      {
        plan_id: plan.id,
        tasks: [
          { title: "First", slug: "first" },
          { title: "Second", slug: "second", spec_path: "specs/second.md" },
          { title: "Third", slug: "third" },
        ],
      },
    );

    expect(result.plan.id).toBe(plan.id);
    expect(result.plan.state).toBe("planned");
    expect(result.tasks.length).toBe(3);
    for (const t of result.tasks) {
      expect(t.state).toBe("draft");
      expect(t.plan_id).toBe(plan.id);
    }
    expect(result.tasks[1]!.spec_path).toBe("specs/second.md");

    const transitions = evidence.filter((e) => e.kind === "transition");
    expect(transitions.length).toBe(4);
    const taskTransitions = transitions.filter((e) => e.task_id !== undefined);
    const planOnlyTransitions = transitions.filter((e) => e.plan_id !== undefined && e.task_id === undefined);
    expect(taskTransitions.length).toBe(3);
    for (const ev of taskTransitions) {
      expect(ev).toMatchObject({
        kind: "transition",
        plan_id: plan.id,
        from_state: null,
        to_state: "draft",
        trigger_verb: "task:from-spec",
      });
    }
    expect(planOnlyTransitions.length).toBe(1);
    expect(planOnlyTransitions[0]).toMatchObject({
      kind: "transition",
      plan_id: plan.id,
      from_state: "specified",
      to_state: "planned",
      trigger_verb: "plan:decompose",
    });
  });

  it("throws ExecPlanNotFoundError when the plan id does not exist", async () => {
    const { planStore, taskStore, evidenceStore, evidence } = makeStores();
    await expect(
      planDecompose(
        { planStore, taskStore, evidenceStore },
        { plan_id: "pln-missing", tasks: [{ title: "A", slug: "a" }] },
      ),
    ).rejects.toBeInstanceOf(ExecPlanNotFoundError);
    expect(evidence.length).toBe(0);
  });

  it("rejects when the plan is not in 'specified' (e.g. already planned)", async () => {
    const { planStore, taskStore, evidenceStore } = makeStores();
    const plan = await seedSpecifiedPlan(planStore);
    await planStore.update(plan.id, { state: "planned" });

    await expect(
      planDecompose(
        { planStore, taskStore, evidenceStore },
        { plan_id: plan.id, tasks: [{ title: "A", slug: "a" }] },
      ),
    ).rejects.toBeInstanceOf(ExecPlanTransitionError);
  });

  it("rejects an empty batch", async () => {
    const { planStore, taskStore, evidenceStore } = makeStores();
    const plan = await seedSpecifiedPlan(planStore);

    await expect(
      planDecompose(
        { planStore, taskStore, evidenceStore },
        { plan_id: plan.id, tasks: [] },
      ),
    ).rejects.toBeInstanceOf(PlanDecomposeBatchEmptyError);
  });

  it("rejects duplicate slugs within the batch", async () => {
    const { planStore, taskStore, evidenceStore } = makeStores();
    const plan = await seedSpecifiedPlan(planStore);

    await expect(
      planDecompose(
        { planStore, taskStore, evidenceStore },
        {
          plan_id: plan.id,
          tasks: [
            { title: "A", slug: "dup" },
            { title: "B", slug: "dup" },
          ],
        },
      ),
    ).rejects.toBeInstanceOf(PlanDecomposeDuplicateSlugInBatchError);
  });

  it("rejects when a batch slug collides with an existing task slug", async () => {
    const { planStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedSpecifiedPlan(planStore);
    await taskStore.create({ slug: "taken", title: "Existing", state: "draft" });

    await expect(
      planDecompose(
        { planStore, taskStore, evidenceStore },
        { plan_id: plan.id, tasks: [{ title: "A", slug: "taken" }] },
      ),
    ).rejects.toBeInstanceOf(DuplicateSlugError);
    // pre-validation should have prevented any task or plan transition writes
    expect(evidence.length).toBe(0);
    expect((await planStore.get(plan.id))!.state).toBe("specified");
  });
});
