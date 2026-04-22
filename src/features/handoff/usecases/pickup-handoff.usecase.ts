import {
  claimTask,
  deriveAgentFromAssignee,
  getUnresolvedBlockerIds,
  loadTaskContinuationSummary,
  syncTaskContinuation,
  transferContractOwnership,
  updateTask,
  type TaskContinuationHistoryPort,
  type TaskContinuationStorePort,
  type TaskStorePort,
} from "@/features/task";
import type { LaunchStorePort, HandoffLaunchRecord } from "@/features/handoff";
import { MaestroError } from "@/shared/errors.js";

export interface PickupHandoffDeps {
  readonly launchStore: LaunchStorePort;
  readonly taskStore: TaskStorePort;
  readonly contractStore: Parameters<typeof transferContractOwnership>[0];
  readonly continuationStore: TaskContinuationStorePort;
  readonly continuationHistory: TaskContinuationHistoryPort;
}

export interface PickupHandoffResult {
  readonly record: HandoffLaunchRecord;
  readonly taskId: string;
  readonly ownerId: string;
  readonly contractTransferWarning?: string;
}

export async function pickupHandoff(
  deps: PickupHandoffDeps,
  input: {
    readonly id: string;
    readonly actorAgent: string;
    readonly actorSessionId?: string;
    readonly ownerId: string;
  },
): Promise<PickupHandoffResult> {
  const launch = await deps.launchStore.get(input.id);
  if (!launch) {
    throw new MaestroError(`Handoff not found: ${input.id}`);
  }
  if (launch.consumedAt) {
    throw new MaestroError(
      `Handoff ${input.id} was already consumed by ${launch.pickedUpByAgent ?? "another agent"} at ${launch.consumedAt}`,
    );
  }

  const taskId = launch.refs.taskId;
  if (!taskId) {
    throw new MaestroError(`Handoff ${input.id} is not linked to a task`, [
      "Create new handoffs from an active task continuation or pass --task-id",
    ]);
  }

  const tasks = new Map((await deps.taskStore.all()).map((task) => [task.id, task] as const));
  const beforeTask = tasks.get(taskId);
  if (!beforeTask) {
    throw new MaestroError(`Linked task not found for handoff ${input.id}: ${taskId}`);
  }
  if (beforeTask.status === "completed") {
    throw new MaestroError(`Task ${taskId} is already completed and cannot be resumed from handoff ${input.id}`, [
      "Reopen the task first if you intend to continue working on it",
    ]);
  }

  const unresolvedBlockers = getUnresolvedBlockerIds(beforeTask, tasks);
  if (unresolvedBlockers.length > 0) {
    throw new MaestroError(`Task ${taskId} is blocked and cannot be resumed`, [
      `Unresolved blockers: ${unresolvedBlockers.join(", ")}`,
    ]);
  }

  const consumed = await deps.launchStore.consume({
    id: input.id,
    agent: input.actorAgent,
    ...(input.actorSessionId ? { sessionId: input.actorSessionId } : {}),
    pickedUpAt: new Date().toISOString(),
  });

  const claimed = await claimTask(deps.taskStore, taskId, {
    sessionId: input.ownerId,
    force: true,
  });
  const resumed = claimed.status === "in_progress"
    ? claimed
    : (await updateTask(
        deps.taskStore,
        taskId,
        { status: "in_progress" },
        { sessionId: input.ownerId, force: true },
      )).task;
  // The handoff was already consumed and the task resumed. Contract ownership
  // transfer is best-effort; the caller surfaces contractTransferWarning so the
  // user sees the failure instead of silently leaving lockedBy out of sync.
  let contractTransferWarning: string | undefined;
  try {
    await transferContractOwnership(deps.contractStore, taskId, input.ownerId, "handoff_pickup");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    contractTransferWarning = `Task ${taskId} was resumed from handoff ${input.id}, but contract ownership transfer failed: ${message}`;
  }

  const priorSummary = await loadTaskContinuationSummary(deps.continuationStore, taskId);
  const priorAgent = deriveAgentFromAssignee(beforeTask.assignee, beforeTask.updatedAt);
  const nextAgent = deriveAgentFromAssignee(resumed.assignee, resumed.updatedAt) ?? {
    type: input.actorAgent,
    ...(input.actorSessionId ? { sessionId: input.actorSessionId } : {}),
    lastSeenAt: resumed.updatedAt,
  };

  await syncTaskContinuation(
    {
      continuationStore: deps.continuationStore,
      continuationHistory: deps.continuationHistory,
    },
    {
      task: resumed,
      summary: {
        currentState: priorSummary?.currentState
          ? `Resumed from handoff ${consumed.id}. ${priorSummary.currentState}`
          : `Resumed from handoff ${consumed.id}.`,
        nextAction: priorSummary?.nextAction ?? `Continue ${resumed.title}.`,
        keyDecisions: priorSummary?.keyDecisions,
        activeAgent: nextAgent,
      },
      event: beforeTask.assignee !== resumed.assignee
        ? {
            kind: "agent_takeover",
            at: resumed.updatedAt,
            summary: `${nextAgent.type} resumed this task from ${priorAgent?.type ?? beforeTask.assignee ?? "the previous owner"}`,
            reason: "handoff_pickup",
            ...(priorAgent ? { from: priorAgent } : {}),
            to: nextAgent,
          }
        : {
            kind: "snapshot",
            at: resumed.updatedAt,
            summary: `Resumed from handoff ${consumed.id}`,
            currentState: priorSummary?.currentState ?? `Resumed from handoff ${consumed.id}.`,
          },
    },
  );
  await deps.continuationHistory.append(taskId, {
    kind: "handoff_picked_up",
    at: resumed.updatedAt,
    summary: `Picked up handoff ${consumed.id}`,
    handoffId: consumed.id,
    agent: input.actorAgent,
    ...(input.actorSessionId ? { sessionId: input.actorSessionId } : {}),
  });

  return {
    record: consumed,
    taskId,
    ownerId: input.ownerId,
    ...(contractTransferWarning ? { contractTransferWarning } : {}),
  };
}
