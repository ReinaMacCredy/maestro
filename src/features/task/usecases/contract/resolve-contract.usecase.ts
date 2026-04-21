import { MaestroError } from "@/shared/errors.js";
import { CONTRACT_ID_PATTERN } from "../../domain/contract/contract-state.js";
import type { Contract } from "../../domain/contract/contract-types.js";
import { isTaskId } from "../../domain/task-id.js";
import type { ContractStoreQueryPort } from "../../ports/contract-store.port.js";

export async function resolveContractRef(
  store: ContractStoreQueryPort,
  ref: string,
): Promise<Contract> {
  const contract = CONTRACT_ID_PATTERN.test(ref)
    ? await store.get(ref)
    : (isTaskId(ref) ? await store.getByTaskId(ref) : undefined);
  if (contract) {
    return contract;
  }

  const noun = CONTRACT_ID_PATTERN.test(ref) ? "Contract" : "Task contract";
  throw new MaestroError(`${noun} ${ref} not found`, [
    "List contracts: maestro task contract list",
    "Use a contract id (c-xxxxxx) or a task id (tsk-xxxxxx)",
  ]);
}
