import type { AgentSlug } from "@/features/session";
import { taskNotFound } from "../domain/task-errors.js";
import type { Task } from "../domain/task-types.js";
import type {
  TaskContinuationAgent,
  TaskContinuationEvent,
  TaskContinuationSummary,
} from "../domain/task-continuation-types.js";
import type { TaskQueryPort } from "../ports/task-store.port.js";
import type { TaskContinuationHistoryPort } from "../ports/task-continuation-history.port.js";
import type { TaskContinuationStorePort } from "../ports/task-continuation-store.port.js";

const KNOWN_AGENT_PREFIXES = [
  "claude-code",
  "opencode",
  "codex",
  "gemini",
  "amp",
  "cline",
  "aider",
  "cursor",
] as const satisfies readonly AgentSlug[];

const NORMALIZED_AGENT_LABELS: Readonly<Record<string, string>> = {
  "claude-code": "claude",
};

export interface TaskContinuationDeps {
  readonly taskStore: TaskQueryPort;
  readonly continuationStore: TaskContinuationStorePort;
  readonly continuationHistory: TaskContinuationHistoryPort;
}

export interface TaskShowView {
  readonly task: Task;
  readonly continuation?: TaskContinuationSummary;
  readonly recentEvents: readonly TaskContinuationEvent[];
}

export interface ContinuationSummaryOverrides {
  readonly lastActiveAt?: string;
  readonly currentState?: string;
  readonly nextAction?: string;
  readonly keyDecisions?: readonly string[];
  readonly activeAgent?: TaskContinuationAgent | null;
}

export interface SyncTaskContinuationInput {
  readonly task: Task;
  readonly summary?: ContinuationSummaryOverrides;
  readonly event?: TaskContinuationEvent;
}

export async function buildTaskShowView(
  deps: TaskContinuationDeps,
  id: string,
): Promise<TaskShowView> {
  const task = await deps.taskStore.get(id);
  if (!task) {
    throw taskNotFound(id);
  }

  const continuation = await loadTaskContinuationSummary(deps.continuationStore, id);
  const recentEvents = continuation
    ? await deps.continuationHistory.listRecent(id, 5)
    : [];

  return { task, continuation, recentEvents };
}

export async function syncTaskContinuation(
  deps: Pick<TaskContinuationDeps, "continuationStore" | "continuationHistory">,
  input: SyncTaskContinuationInput,
): Promise<TaskContinuationSummary> {
  const existing = await loadTaskContinuationSummary(deps.continuationStore, input.task.id);
  const summary = buildTaskContinuationSummary(input.task, existing, input.summary);
  const persisted = input.task.status === "completed"
    ? await deps.continuationStore.archiveCompleted(summary)
    : await deps.continuationStore.upsertActive(summary);

  if (input.event) {
    await deps.continuationHistory.append(input.task.id, input.event);
  }

  return persisted;
}

export async function loadTaskContinuationSummary(
  store: TaskContinuationStorePort,
  taskId: string,
): Promise<TaskContinuationSummary | undefined> {
  return await store.getActive(taskId) ?? await store.getCompleted(taskId);
}

export function buildTaskContinuationSummary(
  task: Task,
  existing?: TaskContinuationSummary,
  overrides: ContinuationSummaryOverrides = {},
): TaskContinuationSummary {
  const activeAgent = overrides.activeAgent === null
    ? undefined
    : overrides.activeAgent ?? deriveAgentFromAssignee(task.assignee, overrides.lastActiveAt ?? task.updatedAt);

  return {
    taskId: task.id,
    status: task.status,
    lastActiveAt: overrides.lastActiveAt ?? task.updatedAt,
    currentState: overrides.currentState ?? deriveCurrentState(task, existing),
    nextAction: overrides.nextAction ?? deriveNextAction(task, existing),
    keyDecisions: overrides.keyDecisions ?? existing?.keyDecisions ?? [],
    ...(activeAgent ? { activeAgent } : {}),
  };
}

export function deriveAgentFromAssignee(
  assignee: string | undefined,
  at: string,
): TaskContinuationAgent | undefined {
  if (!assignee) return undefined;

  const prefix = [...KNOWN_AGENT_PREFIXES]
    .sort((left, right) => right.length - left.length)
    .find((known) => assignee === known || assignee.startsWith(`${known}-`));

  if (prefix) {
    const sessionId = assignee === prefix ? undefined : assignee.slice(prefix.length + 1);
    return {
      type: NORMALIZED_AGENT_LABELS[prefix] ?? prefix,
      ...(sessionId ? { sessionId } : {}),
      lastSeenAt: at,
    };
  }

  const splitAt = assignee.indexOf("-");
  if (splitAt === -1) {
    return {
      type: assignee,
      lastSeenAt: at,
    };
  }

  const type = assignee.slice(0, splitAt);
  const sessionId = assignee.slice(splitAt + 1);
  return {
    type: NORMALIZED_AGENT_LABELS[type] ?? type,
    ...(sessionId ? { sessionId } : {}),
    lastSeenAt: at,
  };
}

function deriveCurrentState(task: Task, existing?: TaskContinuationSummary): string {
  if (task.status === "completed") {
    return task.closeReason
      ? `Completed: ${task.closeReason}`
      : `Completed: ${task.title}`;
  }

  if (existing?.currentState) {
    return existing.currentState;
  }

  if (task.status === "pending") {
    return task.assignee
      ? "Task claimed and ready to start."
      : "Task is pending and waiting to resume.";
  }

  const description = task.description?.trim();
  return description && description.length > 0
    ? description
    : `Working on ${task.title}`;
}

function deriveNextAction(task: Task, existing?: TaskContinuationSummary): string {
  if (task.status === "completed") {
    return "Review the completed task and decide whether follow-up work is needed.";
  }

  if (existing?.nextAction) {
    return existing.nextAction;
  }

  const description = task.description?.trim();
  return description && description.length > 0
    ? description
    : `Continue ${task.title}`;
}
