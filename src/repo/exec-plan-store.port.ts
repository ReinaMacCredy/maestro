import type { ExecPlan, ExecPlanId } from "../types/exec-plan.js";
import type { ExecPlanState } from "../types/exec-plan-state.js";

export interface CreateExecPlanInput {
  readonly slug: string;
  readonly title: string;
  readonly state: ExecPlanState;
  readonly spec_path?: string;
}

export type ExecPlanPatch = Partial<
  Omit<ExecPlan, "id" | "slug" | "created_at" | "updated_at">
>;

export interface ExecPlanStorePort {
  create(input: CreateExecPlanInput): Promise<ExecPlan>;
  get(id: ExecPlanId): Promise<ExecPlan | undefined>;
  update(id: ExecPlanId, patch: ExecPlanPatch): Promise<ExecPlan>;
  list(): Promise<readonly ExecPlan[]>;
  listByState(state: ExecPlanState): Promise<readonly ExecPlan[]>;
}

export class ExecPlanNotFoundError extends Error {
  readonly planId: ExecPlanId;
  constructor(planId: ExecPlanId) {
    super(`Exec-plan ${planId} not found`);
    this.name = "ExecPlanNotFoundError";
    this.planId = planId;
  }
}

export class DuplicateExecPlanSlugError extends Error {
  readonly slug: string;
  constructor(slug: string) {
    super(`Exec-plan with slug ${slug} already exists`);
    this.name = "DuplicateExecPlanSlugError";
    this.slug = slug;
  }
}
