import { isActiveContract } from "../../domain/contract/contract-state.js";
import type { Contract } from "../../domain/contract/contract-types.js";
import type { ContractStorePort } from "../../ports/contract-store.port.js";

export async function transferContractOwnership(
  contractStore: ContractStorePort,
  taskId: string,
  newActor: string,
): Promise<Contract | undefined> {
  const contract = await contractStore.getByTaskId(taskId);
  if (!contract || !isActiveContract(contract) || contract.lockedBy === newActor) {
    return contract;
  }

  return contractStore.save({
    ...contract,
    lockedBy: newActor,
  });
}
