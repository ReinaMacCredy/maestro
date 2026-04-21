import type { Contract } from "../../domain/contract/contract-types.js";
import type { ContractStoreQueryPort } from "../../ports/contract-store.port.js";
import { resolveContractRef } from "./resolve-contract.usecase.js";

export async function showContract(
  contractStore: ContractStoreQueryPort,
  ref: string,
): Promise<Contract> {
  return resolveContractRef(contractStore, ref);
}
