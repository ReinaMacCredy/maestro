import type { ExecPlanState } from "./exec-plan-state.js";

export type ExecPlanId = string;

export interface ExecPlan {
  readonly id: ExecPlanId;
  readonly slug: string;
  readonly title: string;
  readonly state: ExecPlanState;
  readonly spec_path?: string;
  readonly cancel_reason?: string;
  readonly created_at: string;
  readonly updated_at: string;
}

export function generateExecPlanId(): ExecPlanId {
  const rand = Math.random().toString(36).slice(2, 8);
  return `pln-${Date.now().toString(36)}-${rand}`;
}

export const EXEC_PLAN_ID_PATTERN = /^pln-[a-z0-9]+-[a-z0-9]+$/;

export function isExecPlanId(value: unknown): value is ExecPlanId {
  return typeof value === "string" && EXEC_PLAN_ID_PATTERN.test(value);
}
