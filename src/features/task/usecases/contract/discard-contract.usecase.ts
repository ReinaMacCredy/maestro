import { MaestroError } from "@/shared/errors.js";
import { canDiscardContract } from "../../domain/contract/contract-state.js";
import type { Contract } from "../../domain/contract/contract-types.js";
import type { ContractStorePort } from "../../ports/contract-store.port.js";
import { resolveContractRef } from "./resolve-contract.usecase.js";

export async function discardContract(
  contractStore: ContractStorePort,
  ref: string,
): Promise<Contract> {
  const contract = await resolveContractRef(contractStore, ref);
  if (!canDiscardContract(contract)) {
    throw new MaestroError(`Contract ${contract.id} cannot be discarded from status '${contract.status}'`, [
      "Only draft contracts can be discarded",
      `Show the contract: maestro task contract show ${contract.id}`,
    ]);
  }

  return contractStore.save({
    ...contract,
    status: "discarded",
    discardedAt: new Date().toISOString(),
  });
}
