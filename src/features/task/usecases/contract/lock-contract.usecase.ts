import { MaestroError } from "@/shared/errors.js";
import { isContractLockable } from "../../domain/contract/contract-state.js";
import type { Contract } from "../../domain/contract/contract-types.js";
import type { ContractStorePort } from "../../ports/contract-store.port.js";
import { resolveContractRef } from "./resolve-contract.usecase.js";

export interface LockContractInput {
  readonly ref: string;
  readonly actorId: string;
  readonly claimedAtCommit?: string;
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

  const overlapping = (await contractStore.all()).filter((candidate) =>
    candidate.id !== contract.id
    && (candidate.status === "locked" || candidate.status === "amended")
    && candidate.repoRoot === contract.repoRoot,
  );
  if (overlapping.length > 0 && contract.configSnapshot.overlapPolicy === "fail") {
    throw new MaestroError(
      `Contract ${contract.id} overlaps an active contract in the same repo: ${overlapping.map((item) => item.id).join(", ")}`,
      [
        "Discard or finish the other contract first",
        "Or switch contracts.overlapPolicy to annotate if you intentionally allow overlap",
      ],
    );
  }

  const now = new Date().toISOString();
  return contractStore.save({
    ...contract,
    status: "locked",
    lockedAt: now,
    lockedBy: input.actorId,
    claimedAtCommit: input.claimedAtCommit ?? contract.claimedAtCommit,
  });
}
