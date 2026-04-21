import { isActiveContract } from "../../domain/contract/contract-state.js";
import type { Contract } from "../../domain/contract/contract-types.js";
import type { ContractStorePort } from "../../ports/contract-store.port.js";

export async function transferContractOwnership(
  contractStore: ContractStorePort,
  taskId: string,
  newActor: string,
  reason: "claim_reclaim" | "handoff_pickup" = "claim_reclaim",
): Promise<Contract | undefined> {
  const contract = await contractStore.getByTaskId(taskId);
  if (!contract || !isActiveContract(contract) || contract.lockedBy === newActor) {
    return contract;
  }

  const shouldRecordHistory = contract.lockedBy !== undefined
    && !(contract.lockedBy === contract.createdBy && contract.createdBy === "user" && (contract.ownershipHistory?.length ?? 0) === 0);

  return contractStore.save({
    ...contract,
    lockedBy: newActor,
    ...(shouldRecordHistory
      ? {
          ownershipHistory: [
            ...(contract.ownershipHistory ?? []),
            {
              from: contract.lockedBy ?? contract.createdBy,
              to: newActor,
              at: new Date().toISOString(),
              reason,
            },
          ],
        }
      : {}),
  });
}
