import { MaestroError } from "@/shared/errors.js";
import {
  canReopenContract,
  isActiveContract,
} from "../../domain/contract/contract-state.js";
import type { Task } from "../../domain/task-types.js";
import type { Contract } from "../../domain/contract/contract-types.js";
import type { ContractStorePort } from "../../ports/contract-store.port.js";

export async function loadContractForReopen(
  contractStore: ContractStorePort,
  task: Pick<Task, "id" | "contractId">,
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
  if (!canReopenContract(contract)) {
    return contract;
  }

  if (contract.configSnapshot.overlapPolicy === "fail") {
    const overlapping = (await contractStore.all()).filter((candidate) =>
      candidate.id !== contract.id
      && candidate.repoRoot === contract.repoRoot
      && isActiveContract(candidate),
    );
    if (overlapping.length > 0) {
      throw new MaestroError(
        `Contract ${contract.id} overlaps an active contract in the same repo: ${overlapping.map((item) => item.id).join(", ")}`,
        [
          "Discard or finish the other contract first",
          "Or set contracts.overlapPolicy: annotate before reopening intentionally overlapping work",
        ],
      );
    }
  }

  return contract;
}

export async function reopenContractForTask(
  contractStore: ContractStorePort,
  task: Pick<Task, "id" | "contractId">,
): Promise<Contract | undefined> {
  const contract = await loadContractForReopen(contractStore, task);
  if (!contract) {
    return undefined;
  }
  if (!canReopenContract(contract)) {
    return contract;
  }

  return contractStore.save({
    ...contract,
    status: "locked",
    closedAt: undefined,
    closedAtCommit: undefined,
    closedBy: undefined,
    verdict: undefined,
  });
}
