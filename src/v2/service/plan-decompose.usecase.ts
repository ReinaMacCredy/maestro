import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { ExecPlanStorePort } from "../repo/exec-plan-store.port.js";
import { ExecPlanNotFoundError } from "../repo/exec-plan-store.port.js";
import type { ObservabilityPort } from "../repo/observability.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { DuplicateSlugError } from "../repo/task-store.port.js";
import type { ExecPlan, ExecPlanId } from "../types/exec-plan.js";
import { assertExecPlanTransition } from "../types/exec-plan-state.js";
import type { Task } from "../types/task.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";

export interface PlanDecomposeTaskInput {
  readonly title: string;
  readonly slug: string;
  readonly spec_path?: string;
}

export interface PlanDecomposeDeps {
  readonly planStore: ExecPlanStorePort;
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly observabilityStore?: ObservabilityPort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface PlanDecomposeInput {
  readonly plan_id: ExecPlanId;
  readonly tasks: readonly PlanDecomposeTaskInput[];
}

export interface PlanDecomposeResult {
  readonly plan: ExecPlan;
  readonly tasks: readonly Task[];
}

export class PlanDecomposeBatchEmptyError extends Error {
  constructor() {
    super("plan decompose requires at least one task in the batch");
    this.name = "PlanDecomposeBatchEmptyError";
  }
}

export class PlanDecomposeBatchInvalidError extends Error {
  readonly index: number;
  readonly field: string;
  constructor(index: number, field: string, detail: string) {
    super(`plan decompose: task[${index}].${field} ${detail}`);
    this.name = "PlanDecomposeBatchInvalidError";
    this.index = index;
    this.field = field;
  }
}

export class PlanDecomposeDuplicateSlugInBatchError extends Error {
  readonly slug: string;
  constructor(slug: string) {
    super(`plan decompose: slug '${slug}' appears more than once in the batch`);
    this.name = "PlanDecomposeDuplicateSlugInBatchError";
    this.slug = slug;
  }
}

export function parsePlanDecomposeBatch(raw: unknown): readonly PlanDecomposeTaskInput[] {
  const arr = Array.isArray(raw)
    ? raw
    : raw !== null &&
        typeof raw === "object" &&
        Array.isArray((raw as { tasks?: unknown }).tasks)
      ? (raw as { tasks: unknown[] }).tasks
      : undefined;
  if (!arr) {
    throw new PlanDecomposeBatchInvalidError(
      -1,
      "root",
      "must be a JSON array, or an object with a 'tasks' array",
    );
  }
  if (arr.length === 0) throw new PlanDecomposeBatchEmptyError();
  const tasks: PlanDecomposeTaskInput[] = [];
  for (let i = 0; i < arr.length; i += 1) {
    const t = arr[i];
    if (t === null || typeof t !== "object") {
      throw new PlanDecomposeBatchInvalidError(i, "self", "must be an object");
    }
    const rec = t as Record<string, unknown>;
    if (typeof rec.title !== "string" || rec.title.trim().length === 0) {
      throw new PlanDecomposeBatchInvalidError(i, "title", "must be a non-empty string");
    }
    if (typeof rec.slug !== "string" || rec.slug.trim().length === 0) {
      throw new PlanDecomposeBatchInvalidError(i, "slug", "must be a non-empty string");
    }
    if (
      rec.spec_path !== undefined &&
      (typeof rec.spec_path !== "string" || rec.spec_path.length === 0)
    ) {
      throw new PlanDecomposeBatchInvalidError(i, "spec_path", "must be a non-empty string if set");
    }
    tasks.push({
      title: rec.title,
      slug: rec.slug,
      spec_path: rec.spec_path as string | undefined,
    });
  }
  return tasks;
}

export async function planDecompose(
  deps: PlanDecomposeDeps,
  input: PlanDecomposeInput,
): Promise<PlanDecomposeResult> {
  if (input.tasks.length === 0) throw new PlanDecomposeBatchEmptyError();
  const seen = new Set<string>();
  for (const t of input.tasks) {
    if (seen.has(t.slug)) throw new PlanDecomposeDuplicateSlugInBatchError(t.slug);
    seen.add(t.slug);
  }

  const plan = await deps.planStore.get(input.plan_id);
  if (!plan) throw new ExecPlanNotFoundError(input.plan_id);
  assertExecPlanTransition(plan.state, "planned");

  const existing = await deps.taskStore.list();
  for (const t of input.tasks) {
    if (existing.some((e) => e.slug === t.slug)) {
      throw new DuplicateSlugError(t.slug);
    }
  }

  const created: Task[] = [];
  for (const t of input.tasks) {
    const task = await deps.taskStore.create({
      slug: t.slug,
      title: t.title,
      state: "draft",
      spec_path: t.spec_path,
      plan_id: plan.id,
    });
    created.push(task);
    await emitTransitionEvidence(
      {
        store: deps.evidenceStore,
        observabilityStore: deps.observabilityStore,
        clock: deps.clock,
        idFactory: deps.idFactory,
      },
      {
        task_id: task.id,
        plan_id: plan.id,
        from_state: null,
        to_state: "draft",
        trigger_verb: "task:from-spec",
      },
    );
  }

  const updatedPlan = await deps.planStore.update(plan.id, { state: "planned" });
  await emitTransitionEvidence(
    {
      store: deps.evidenceStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    {
      plan_id: plan.id,
      from_state: plan.state,
      to_state: "planned",
      trigger_verb: "plan:decompose",
    },
  );

  return { plan: updatedPlan, tasks: created };
}
