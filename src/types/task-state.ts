// Task lifecycle (ADR-0003). Hybrid transitions (ADR-0004): agent enters
// check states manually; harness auto-exits based on verdict result.

export const TASK_STATES = [
  "draft",
  "claimed",
  "doing",
  "verifying",
  "blocked",
  "ready",
  "shipped",
  "abandoned",
] as const;

export type TaskState = (typeof TASK_STATES)[number];

export const TASK_TERMINAL_STATES = ["shipped", "abandoned"] as const;
export type TerminalTaskState = (typeof TASK_TERMINAL_STATES)[number];

export const TASK_TRANSITIONS = {
  draft: ["claimed", "abandoned"],
  claimed: ["doing", "verifying", "blocked", "abandoned"],
  doing: ["verifying", "blocked", "abandoned"],
  verifying: ["doing", "ready", "blocked", "abandoned"],
  blocked: ["doing", "verifying", "abandoned"],
  // `verifying` re-entry: zero-diff PASS on a task that wasn't actually done
  // strands the agent if `ready` can't go back. Re-verification is intended
  // (see task-verify.usecase.ts:69) — the gate is human ship/abandon, not the
  // transition itself.
  ready: ["shipped", "abandoned", "verifying"],
  shipped: [],
  abandoned: [],
} as const satisfies Record<TaskState, readonly TaskState[]>;

export function isTaskState(value: unknown): value is TaskState {
  return typeof value === "string" && (TASK_STATES as readonly string[]).includes(value);
}

export function isTerminalTaskState(state: TaskState): state is TerminalTaskState {
  return (TASK_TERMINAL_STATES as readonly TaskState[]).includes(state);
}

export function canTransitionTask(from: TaskState, to: TaskState): boolean {
  return (TASK_TRANSITIONS[from] as readonly TaskState[]).includes(to);
}

export class TaskTransitionError extends Error {
  readonly from: TaskState;
  readonly to: TaskState;
  readonly allowed: readonly TaskState[];

  constructor(from: TaskState, to: TaskState) {
    const allowed = TASK_TRANSITIONS[from];
    const hint =
      allowed.length === 0
        ? `${from} is terminal`
        : `allowed from ${from}: ${allowed.join(", ")}`;
    super(`Invalid task transition ${from} -> ${to} (${hint})`);
    this.name = "TaskTransitionError";
    this.from = from;
    this.to = to;
    this.allowed = allowed;
  }
}

export function assertTaskTransition(from: TaskState, to: TaskState): void {
  if (!canTransitionTask(from, to)) {
    throw new TaskTransitionError(from, to);
  }
}
