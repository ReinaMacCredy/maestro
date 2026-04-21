import type { Contract, ContractStatus } from "../../domain/contract/contract-types.js";
import type { ContractStoreQueryPort } from "../../ports/contract-store.port.js";

export interface ListContractsFilters {
  readonly status?: ContractStatus;
  readonly taskId?: string;
}

export async function listContracts(
  contractStore: ContractStoreQueryPort,
  filters: ListContractsFilters = {},
): Promise<readonly Contract[]> {
  const contracts = await contractStore.all();
  return contracts.filter((contract) => {
    if (filters.status !== undefined && contract.status !== filters.status) {
      return false;
    }
    if (filters.taskId !== undefined && contract.taskId !== filters.taskId) {
      return false;
    }
    return true;
  });
}
