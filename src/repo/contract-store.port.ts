import type {
  Contract,
  ContractIndexEntry,
  CreateContractRecordInput,
  DeleteContractRecordInput,
} from "../types/contract.js";

export interface ContractStoreQueryPort {
  get(id: string): Promise<Contract | undefined>;
  getByTaskId(taskId: string): Promise<Contract | undefined>;
  all(): Promise<readonly Contract[]>;
  readIndex(): Promise<readonly ContractIndexEntry[]>;
}

export interface ContractStorePort extends ContractStoreQueryPort {
  create(input: CreateContractRecordInput): Promise<Contract>;
  save(contract: Contract): Promise<Contract>;
  delete(id: string, input: DeleteContractRecordInput): Promise<boolean>;
}

export interface ContractVersionStorePort {
  write(taskId: string, version: number, contract: Contract): Promise<void>;
  readCurrent(taskId: string): Promise<Contract | undefined>;
  readVersion(taskId: string, version: number): Promise<Contract | undefined>;
  history(taskId: string): Promise<readonly Contract[]>;
}
