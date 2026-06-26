export interface ContractVersionStorePort {
  readonly readCurrent?: (taskId: string) => Promise<string | null>;
}

export interface ContractStoreQueryPort {
  readonly read?: (id: string) => Promise<unknown>;
}
