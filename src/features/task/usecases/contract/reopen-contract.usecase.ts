import { MaestroError } from "@/shared/errors.js";
import {
  buildActiveOverlapError,
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
      && isActiveContract(candidate),
    );
    if (overlapping.length > 0) {
      throw buildActiveOverlapError(contract.id, overlapping.map((item) => item.id));
    }
  }

  return contract;
}

export async function reopenContractForTask(
  contractStore: ContractStorePort,
  task: Pick<Task, "id" | "contractId">,
  loadedContract?: Contract,
): Promise<Contract | undefined> {
  const contract = loadedContract ?? await loadContractForReopen(contractStore, task);
  if (!contract) {
    return undefined;
  }

  return reopenLoadedContract(contractStore, contract);
}

async function reopenLoadedContract(
  contractStore: ContractStorePort,
  contract: Contract,
): Promise<Contract> {
  if (!canReopenContract(contract)) {
    return contract;
  }

  return contractStore.save({
    ...contract,
    status: contract.amendments.length > 0 ? "amended" : "locked",
    closedAt: undefined,
    closedAtCommit: undefined,
    closedBy: undefined,
    verdict: undefined,
  });
}
