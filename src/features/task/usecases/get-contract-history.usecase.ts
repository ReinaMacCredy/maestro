import type { Contract } from "../domain/contract/contract-types.js";
import type { ContractVersionStorePort } from "../ports/contract-version-store.port.js";

export async function getContractHistory(
  store: ContractVersionStorePort,
  taskId: string,
): Promise<readonly Contract[]> {
  return store.history(taskId);
}
