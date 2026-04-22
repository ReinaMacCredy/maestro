import { MaestroError } from "@/shared/errors.js";
import {
  generateDoneWhenId,
  normalizeStoredContractRepoRoot,
} from "../../domain/contract/contract-state.js";
import type {
  Contract,
  ContractConfigSnapshot,
  ContractScope,
  DoneWhenCriterion,
} from "../../domain/contract/contract-types.js";
import { taskAlreadyCompleted, taskNotFound } from "../../domain/task-errors.js";
import type { ContractStorePort } from "../../ports/contract-store.port.js";
import type { TaskStorePort } from "../../ports/task-store.port.js";
import { syncTaskMetadata } from "../sync-task-metadata.usecase.js";
import { normalizeScope } from "./amend-contract.usecase.js";

export interface CreateContractInput {
  readonly taskId: string;
  readonly repoRoot: string;
  readonly intent: string;
  readonly scope: ContractScope;
  readonly doneWhen: readonly Array<{
    readonly text: string;
    readonly kind?: DoneWhenCriterion["kind"];
  }>;
  readonly createdBy: string;
  readonly configSnapshot: ContractConfigSnapshot;
}

export async function createContract(
  taskStore: TaskStorePort,
  contractStore: ContractStorePort,
  input: CreateContractInput,
): Promise<Contract> {
  const task = await taskStore.get(input.taskId);
  if (!task) {
    throw taskNotFound(input.taskId);
  }
  if (task.status === "completed") {
    throw taskAlreadyCompleted(task.id);
  }
  if (task.contractId) {
    const linked = await contractStore.get(task.contractId);
    if (linked?.status === "discarded") {
      await syncTaskMetadata(taskStore, task.id, { contractId: null });
    } else {
      throw new MaestroError(`Task ${task.id} already has a contract: ${task.contractId}`, [
        `Show it: maestro task contract show ${task.id}`,
        "Discard the draft first if you need to stop using it",
      ]);
    }
  }

  const contract = await contractStore.create({
    taskId: input.taskId,
    repoRoot: normalizeStoredContractRepoRoot(input.repoRoot),
    createdAt: new Date().toISOString(),
    intent: input.intent.trim(),
    scope: normalizeScope(input.scope),
    doneWhen: input.doneWhen.map((criterion) => ({
      id: generateDoneWhenId(),
      text: criterion.text.trim(),
      kind: criterion.kind ?? "manual",
    })),
    createdBy: input.createdBy,
    configSnapshot: input.configSnapshot,
  });

  try {
    await syncTaskMetadata(taskStore, task.id, { contractId: contract.id });
  } catch (error) {
    await contractStore.delete(contract.id, {
      taskId: contract.taskId,
      at: new Date().toISOString(),
      reason: "task_link_failed",
    });
    throw error;
  }

  return contract;
}
