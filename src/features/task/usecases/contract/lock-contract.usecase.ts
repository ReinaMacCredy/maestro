import { MaestroError } from "@/shared/errors.js";
import { isContractLockable } from "../../domain/contract/contract-state.js";
import type { Contract, ContractConfigSnapshot } from "../../domain/contract/contract-types.js";
import type { ContractStorePort } from "../../ports/contract-store.port.js";
import { resolveContractRef } from "./resolve-contract.usecase.js";

export interface LockContractInput {
  readonly ref: string;
  readonly actorId: string;
  readonly claimedAtCommit?: string;
  readonly configSnapshot: ContractConfigSnapshot;
}

export async function lockContract(
  contractStore: ContractStorePort,
  input: LockContractInput,
): Promise<Contract> {
  const contract = await resolveContractRef(contractStore, input.ref);
  if (!isContractLockable(contract)) {
    throw new MaestroError(`Contract ${contract.id} cannot be locked from status '${contract.status}'`, [
      "Draft contracts need a non-empty intent, at least one expected file glob, and at least one done-when criterion",
      `Show the draft: maestro task contract show ${contract.id}`,
    ]);
  }

  const now = new Date().toISOString();
  return contractStore.save({
    ...contract,
    status: "locked",
    lockedAt: now,
    lockedBy: input.actorId,
    claimedAtCommit: input.claimedAtCommit ?? contract.claimedAtCommit,
    configSnapshot: input.configSnapshot,
  });
}
