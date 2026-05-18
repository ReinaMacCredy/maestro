import { MaestroError } from "@/shared/errors.js";
import type { Contract } from "../domain/contract/contract-types.js";
import type { ContractVersionStorePort } from "../ports/contract-version-store.port.js";

export async function proposeContract(
  store: ContractVersionStorePort,
  contract: Contract,
): Promise<void> {
  const existing = await store.readVersion(contract.taskId, 1);
  if (existing !== undefined) {
    throw new MaestroError(
      `Contract for task ${contract.taskId} already has a v1 — use amendContract to evolve it`,
      ["A versioned contract can only be proposed once per task"],
    );
  }
  await store.write(contract.taskId, 1, contract);
}
