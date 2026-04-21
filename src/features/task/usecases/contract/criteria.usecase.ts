import { MaestroError } from "@/shared/errors.js";
import { generateDoneWhenId } from "../../domain/contract/contract-state.js";
import type { Contract, DoneWhenCriterion } from "../../domain/contract/contract-types.js";
import type { ContractStorePort } from "../../ports/contract-store.port.js";
import { resolveActiveContract, withContractAmendment } from "./amend-contract.usecase.js";

export interface AddContractCriterionInput {
  readonly ref: string;
  readonly actorId: string;
  readonly text: string;
}

export interface RemoveContractCriterionInput {
  readonly ref: string;
  readonly actorId: string;
  readonly criterionId: string;
}

export interface MarkContractCriterionInput {
  readonly ref: string;
  readonly actorId: string;
  readonly criterionId: string;
  readonly met?: boolean;
  readonly evidence?: string;
}

export async function addContractCriterion(
  contractStore: ContractStorePort,
  input: AddContractCriterionInput,
): Promise<Contract> {
  const contract = await resolveActiveContract(contractStore, input.ref);
  const text = input.text.trim();
  if (text.length === 0) {
    throw new MaestroError("Contract criteria need non-empty text");
  }

  const nextCriterion: DoneWhenCriterion = {
    id: generateDoneWhenId(),
    text,
    kind: "manual",
  };
  return contractStore.save(
    withContractAmendment(contract, {
      actorId: input.actorId,
      reason: `Added criterion ${nextCriterion.id}`,
      doneWhen: [...contract.doneWhen, nextCriterion],
    }),
  );
}

export async function removeContractCriterion(
  contractStore: ContractStorePort,
  input: RemoveContractCriterionInput,
): Promise<Contract> {
  const contract = await resolveActiveContract(contractStore, input.ref);
  const criterion = findCriterion(contract, input.criterionId);
  return contractStore.save(
    withContractAmendment(contract, {
      actorId: input.actorId,
      reason: `Removed criterion ${criterion.id}`,
      doneWhen: contract.doneWhen.filter((candidate) => candidate.id !== criterion.id),
    }),
  );
}

export async function markContractCriterion(
  contractStore: ContractStorePort,
  input: MarkContractCriterionInput,
): Promise<Contract> {
  const contract = await resolveActiveContract(contractStore, input.ref);
  const criterion = findCriterion(contract, input.criterionId);
  const met = input.met ?? true;
  const evidence = input.evidence?.trim();
  if (!met && evidence) {
    throw new MaestroError("--evidence only applies when marking a criterion met");
  }

  const at = new Date().toISOString();
  const nextCriterion = met
    ? {
        ...criterion,
        met: true,
        metAt: at,
        metBy: input.actorId,
        ...(evidence ? { metEvidence: evidence } : {}),
      }
    : {
        id: criterion.id,
        text: criterion.text,
        kind: criterion.kind,
      };

  return contractStore.save(
    withContractAmendment(contract, {
      actorId: input.actorId,
      reason: `Marked criterion ${criterion.id} ${met ? "met" : "unmet"}`,
      at,
      doneWhen: contract.doneWhen.map((candidate) => candidate.id === criterion.id ? nextCriterion : candidate),
    }),
  );
}

function findCriterion(contract: Contract, criterionId: string): DoneWhenCriterion {
  const criterion = contract.doneWhen.find((candidate) => candidate.id === criterionId);
  if (criterion) {
    return criterion;
  }
  throw new MaestroError(`Criterion ${criterionId} not found on contract ${contract.id}`, [
    `Show the contract: maestro task contract show ${contract.id}`,
  ]);
}
