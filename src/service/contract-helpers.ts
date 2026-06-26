import type {
  ContractStoreQueryPort,
  ContractVersionStorePort,
} from "@/repo/contract-store.port.js";

export interface CurrentContract {
  readonly intent?: string;
  readonly costBudget?: {
    readonly maxRetries?: number;
    readonly maxWallClockSeconds?: number;
  };
}

export async function readCurrentContractWithBackfill(
  _versionStore: ContractVersionStorePort,
  _contractStore: ContractStoreQueryPort,
  _taskId: string,
): Promise<CurrentContract | null> {
  return null;
}
