// v2 exec-plan lifecycle (ADR-0003). Auto-completes when every child task is
// in a terminal state (ADR-0011); cancellation is the only manual terminal.

export const EXEC_PLAN_STATES = [
  "intake",
  "specified",
  "planned",
  "in-progress",
  "completed",
  "cancelled",
] as const;

export type ExecPlanState = (typeof EXEC_PLAN_STATES)[number];

export const EXEC_PLAN_TERMINAL_STATES = ["completed", "cancelled"] as const;
export type TerminalExecPlanState = (typeof EXEC_PLAN_TERMINAL_STATES)[number];

export const EXEC_PLAN_TRANSITIONS = {
  intake: ["specified", "cancelled"],
  specified: ["planned", "cancelled"],
  planned: ["in-progress", "cancelled"],
  "in-progress": ["completed", "cancelled"],
  completed: [],
  cancelled: [],
} as const satisfies Record<ExecPlanState, readonly ExecPlanState[]>;

export function isExecPlanState(value: unknown): value is ExecPlanState {
  return typeof value === "string" && (EXEC_PLAN_STATES as readonly string[]).includes(value);
}

export function isTerminalExecPlanState(state: ExecPlanState): state is TerminalExecPlanState {
  return (EXEC_PLAN_TERMINAL_STATES as readonly ExecPlanState[]).includes(state);
}

export function canTransitionExecPlan(from: ExecPlanState, to: ExecPlanState): boolean {
  return (EXEC_PLAN_TRANSITIONS[from] as readonly ExecPlanState[]).includes(to);
}

export class ExecPlanTransitionError extends Error {
  readonly from: ExecPlanState;
  readonly to: ExecPlanState;
  readonly allowed: readonly ExecPlanState[];

  constructor(from: ExecPlanState, to: ExecPlanState) {
    const allowed = EXEC_PLAN_TRANSITIONS[from];
    const hint =
      allowed.length === 0
        ? `${from} is terminal`
        : `allowed from ${from}: ${allowed.join(", ")}`;
    super(`Invalid exec-plan transition ${from} -> ${to} (${hint})`);
    this.name = "ExecPlanTransitionError";
    this.from = from;
    this.to = to;
    this.allowed = allowed;
  }
}

export function assertExecPlanTransition(from: ExecPlanState, to: ExecPlanState): void {
  if (!canTransitionExecPlan(from, to)) {
    throw new ExecPlanTransitionError(from, to);
  }
}
