import type { Contract } from "../domain/contract/contract-types.js";

export interface ContractVersionStorePort {
  write(taskId: string, version: number, contract: Contract): Promise<void>;
  readCurrent(taskId: string): Promise<Contract | undefined>;
  readVersion(taskId: string, version: number): Promise<Contract | undefined>;
  history(taskId: string): Promise<readonly Contract[]>;
}
