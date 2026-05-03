import type { Contract } from "../domain/contract/contract-types.js";
import type { ContractVersionStorePort } from "../ports/contract-version-store.port.js";

export async function getCurrentContract(
  store: ContractVersionStorePort,
  taskId: string,
): Promise<Contract | undefined> {
  return store.readCurrent(taskId);
}
