import { MaestroError } from "@/shared/errors.js";
import { canCloseContract } from "../../domain/contract/contract-state.js";
import type { Task } from "../../domain/task-types.js";
import type { Contract } from "../../domain/contract/contract-types.js";
import type { ContractStorePort } from "../../ports/contract-store.port.js";
import type { GitAnchorPort } from "../../ports/git-anchor.port.js";
import { computeContractVerdictForTask } from "./compute-verdict.usecase.js";

export async function closeContractForTask(
  contractStore: ContractStorePort,
  gitAnchor: GitAnchorPort,
  task: Task,
): Promise<Contract | undefined> {
  if (!task.contractId) {
    return undefined;
  }

  const contract = await contractStore.get(task.contractId);
  if (!contract) {
    throw new MaestroError(`Contract ${task.contractId} not found for task ${task.id}`, [
      "Inspect the contract index under .maestro/tasks/contracts/",
    ]);
  }
  if (contract.status === "discarded") {
    return contract;
  }
  if (contract.status === "fulfilled" || contract.status === "broken") {
    return contract;
  }
  if (!canCloseContract(contract)) {
    throw new MaestroError(`Contract ${contract.id} must be locked before task completion`, [
      `Lock it first: maestro task contract lock ${contract.id}`,
    ]);
  }

  const computed = await computeContractVerdictForTask(gitAnchor, contract, task);
  return contractStore.save({
    ...contract,
    status: computed.verdict.fulfilled ? "fulfilled" : "broken",
    closedAt: task.updatedAt,
    closedAtCommit: computed.closedAtCommit,
    closedBy: task.assignee ?? contract.lockedBy ?? contract.createdBy,
    doneWhen: computed.criteria,
    verdict: computed.verdict,
  });
}
