import {
  claimTask,
  deriveAgentFromAssignee,
  getUnresolvedBlockerIds,
  loadTaskContinuationSummary,
  syncTaskContinuation,
  updateTask,
  type ContractWorkflows,
  type TaskContinuationHistoryPort,
  type TaskContinuationStorePort,
  type TaskStorePort,
} from "@/features/task";
import type { HandoffStorePort, HandoffRecord } from "@/features/handoff";
import { MaestroError } from "@/shared/errors.js";
import { resolveHandoffProjectRoot } from "../domain/project-scope.js";
import { reconcileHandoffRecord } from "./reconcile-handoff-record.usecase.js";

export interface PickupHandoffDeps {
  readonly handoffStore: HandoffStorePort;
  readonly taskStore: TaskStorePort;
  readonly contracts: ContractWorkflows;
  readonly continuationStore: TaskContinuationStorePort;
  readonly continuationHistory: TaskContinuationHistoryPort;
}

export interface PickupHandoffResult {
  readonly record: HandoffRecord;
  readonly taskId?: string;
  readonly ownerId?: string;
  readonly contractTransferWarning?: string;
  readonly unlinkedTaskId?: string;
}

export async function pickupHandoff(
  deps: PickupHandoffDeps,
  input: {
    readonly id: string;
    readonly actorAgent: string;
    readonly actorSessionId?: string;
    readonly ownerId?: string;
    readonly currentProjectRoot?: string;
    readonly standalone?: boolean;
  },
): Promise<PickupHandoffResult> {
  const storedLaunch = await deps.handoffStore.get(input.id);
  if (!storedLaunch) {
    throw new MaestroError(`Handoff not found: ${input.id}`);
  }
  const launch = await reconcileHandoffRecord(
    {
      handoffStore: deps.handoffStore,
      taskStore: deps.taskStore,
      currentProjectRoot: input.currentProjectRoot,
    },
    storedLaunch,
  );
  if (launch.consumedAt) {
    throw new MaestroError(
      `Handoff ${input.id} was already consumed by ${launch.pickedUpByAgent ?? "another agent"} at ${launch.consumedAt}`,
    );
  }

  const taskId = launch.refs.taskId;
  if (!taskId || input.standalone) {
    const consumedOnly = await consumeHandoffOnly(deps.handoffStore, input);
    return {
      record: consumedOnly,
    };
  }
  if (launch.status === "completed") {
    throw new MaestroError(
      `Handoff ${input.id} is already finished because linked task ${taskId} is completed`,
      [
        `Inspect the packet with: maestro handoff show ${input.id}`,
        "Reopen the task and create a fresh handoff if more work is needed",
      ],
    );
  }

  const sourceProjectRoot = resolveHandoffProjectRoot(launch);
  if (input.currentProjectRoot && input.currentProjectRoot !== sourceProjectRoot) {
    throw new MaestroError(
      `Handoff ${input.id} belongs to project ${sourceProjectRoot} and remains linked to task ${taskId}`,
      [
        `Pick it up from the source project to preserve task ownership: ${buildSourceProjectPickupCommand(sourceProjectRoot, input.id)}`,
        `Or consume it here as prompt-only: maestro handoff pickup --id ${input.id} --standalone --json`,
      ],
    );
  }

  const ownerId = input.ownerId;
  if (!ownerId) {
    throw new MaestroError(`Pickup for task-linked handoff ${input.id} requires an ownerId`);
  }

  const tasks = new Map((await deps.taskStore.all()).map((task) => [task.id, task] as const));
  const beforeTask = tasks.get(taskId);
  if (!beforeTask) {
    const consumedOnly = await consumeHandoffOnly(deps.handoffStore, input);
    return {
      record: consumedOnly,
      unlinkedTaskId: taskId,
    };
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

  const consumed = await deps.handoffStore.consume({
    id: input.id,
    agent: input.actorAgent,
    ...(input.actorSessionId ? { sessionId: input.actorSessionId } : {}),
    pickedUpAt: new Date().toISOString(),
  });

  const claimed = await claimTask(deps.taskStore, taskId, {
    sessionId: ownerId,
    force: true,
  });
  const resumed = claimed.status === "in_progress"
    ? claimed
    : (await updateTask(
        deps.taskStore,
        taskId,
        { status: "in_progress" },
        { sessionId: ownerId, force: true },
      )).task;
  // The handoff was already consumed and the task resumed. Contract ownership
  // transfer is best-effort; the caller surfaces contractTransferWarning so the
  // user sees the failure instead of silently leaving lockedBy out of sync.
  let contractTransferWarning: string | undefined;
  try {
    await deps.contracts.transferOwnership(taskId, ownerId, "handoff_pickup");
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
    ownerId,
    ...(contractTransferWarning ? { contractTransferWarning } : {}),
  };
}

async function consumeHandoffOnly(
  handoffStore: HandoffStorePort,
  input: {
    readonly id: string;
    readonly actorAgent: string;
    readonly actorSessionId?: string;
  },
): Promise<HandoffRecord> {
  return await handoffStore.consume({
    id: input.id,
    agent: input.actorAgent,
    ...(input.actorSessionId ? { sessionId: input.actorSessionId } : {}),
    pickedUpAt: new Date().toISOString(),
  });
}

function buildSourceProjectPickupCommand(projectRoot: string, handoffId: string): string {
  return `cd ${JSON.stringify(projectRoot)} && maestro handoff pickup --id ${handoffId} --json`;
}
