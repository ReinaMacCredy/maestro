import { taskNotFound } from "../domain/task-errors.js";
import type { Contract } from "../domain/contract/contract-types.js";
import type { Task } from "../domain/task-types.js";
import type { ContractStorePort } from "../ports/contract-store.port.js";
import type { TaskContinuationHistoryPort } from "../ports/task-continuation-history.port.js";
import type { TaskContinuationStorePort } from "../ports/task-continuation-store.port.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import {
  buildTaskContinuationSummary,
  loadTaskContinuationSummary,
} from "./task-continuation.usecase.js";
import {
  loadContractForReopen,
  reopenContractForTask,
} from "./contract/reopen-contract.usecase.js";

export interface ReopenTaskFlowDeps {
  readonly taskStore: TaskStorePort;
  readonly continuationStore: TaskContinuationStorePort;
  readonly continuationHistory: TaskContinuationHistoryPort;
  readonly contractStore: ContractStorePort;
}

export interface ReopenTaskFlowResult {
  readonly task: Task;
  readonly contract?: Contract;
}

export async function reopenTaskFlow(
  deps: ReopenTaskFlowDeps,
  taskId: string,
): Promise<ReopenTaskFlowResult> {
  const previous = await deps.taskStore.get(taskId);
  if (!previous) {
    throw taskNotFound(taskId);
  }
  const contractForReopen = await loadContractForReopen(deps.contractStore, previous);
  const contract = contractForReopen
    ? await reopenContractForTask(deps.contractStore, previous, contractForReopen)
    : undefined;

  let reopened: Task;
  try {
    reopened = await deps.taskStore.reopen(taskId);
  } catch (error) {
    if (contractForReopen && contract) {
      await deps.contractStore.save(contractForReopen).catch(() => {});
    }
    throw error;
  }
  const existingSummary = await loadTaskContinuationSummary(deps.continuationStore, taskId);
  const summary = buildTaskContinuationSummary(reopened, existingSummary, {
    currentState: "Task reopened and ready to resume.",
    nextAction: `Resume ${reopened.title}.`,
    activeAgent: null,
  });

  const [restored] = await Promise.all([
    deps.continuationStore.reopen(taskId, summary),
    deps.continuationHistory.append(taskId, {
      kind: "task_reopened",
      at: reopened.updatedAt,
      summary: previous.closeReason
        ? `Reopened after completion: ${previous.closeReason}`
        : "Reopened and returned to pending",
      ...(previous.closeReason ? { reason: previous.closeReason } : {}),
    }),
  ]);
  if (!restored) {
    await deps.continuationStore.upsertActive(summary);
  }
  return { task: reopened, contract };
}
