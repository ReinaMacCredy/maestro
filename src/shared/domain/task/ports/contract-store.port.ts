import type {
  Contract,
  ContractIndexEntry,
  CreateContractRecordInput,
  DeleteContractRecordInput,
} from "../domain/contract/contract-types.js";

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
