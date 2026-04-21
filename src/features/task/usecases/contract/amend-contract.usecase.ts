import { MaestroError } from "@/shared/errors.js";
import {
  canAmendContract,
  generateContractAmendmentId,
  generateDoneWhenId,
  snapshotForAmendment,
} from "../../domain/contract/contract-state.js";
import type {
  Contract,
  ContractScope,
  DoneWhenCriterion,
} from "../../domain/contract/contract-types.js";
import type { ContractStorePort } from "../../ports/contract-store.port.js";
import { resolveContractRef } from "./resolve-contract.usecase.js";

export interface ContractCriterionDraftInput {
  readonly id?: string;
  readonly text: string;
  readonly kind?: DoneWhenCriterion["kind"];
}

export interface AmendContractInput {
  readonly ref: string;
  readonly actorId: string;
  readonly reason: string;
  readonly intent: string;
  readonly scope: ContractScope;
  readonly doneWhen: readonly ContractCriterionDraftInput[];
}

export async function amendContract(
  contractStore: ContractStorePort,
  input: AmendContractInput,
): Promise<Contract> {
  const contract = await resolveActiveContract(contractStore, input.ref);
  const nextIntent = input.intent.trim();
  const nextScope = normalizeScope(input.scope);
  const nextDoneWhen = normalizeAmendedCriteria(contract.doneWhen, input.doneWhen);

  return contractStore.save(
    withContractAmendment(contract, {
      actorId: input.actorId,
      reason: input.reason,
      intent: nextIntent,
      scope: nextScope,
      doneWhen: nextDoneWhen,
    }),
  );
}

export function withContractAmendment(
  contract: Contract,
  input: {
    readonly actorId: string;
    readonly reason: string;
    readonly intent?: string;
    readonly scope?: ContractScope;
    readonly doneWhen?: readonly DoneWhenCriterion[];
    readonly at?: string;
  },
): Contract {
  const reason = input.reason.trim();
  if (reason.length === 0) {
    throw new MaestroError("Contract amendments require a non-empty reason", [
      "Pass --reason \"why the contract changed\"",
    ]);
  }

  const at = input.at ?? new Date().toISOString();
  const nextIntent = input.intent ?? contract.intent;
  const nextScope = input.scope ?? contract.scope;
  const nextDoneWhen = input.doneWhen ?? contract.doneWhen;

  return {
    ...contract,
    status: "amended",
    intent: nextIntent,
    scope: nextScope,
    doneWhen: nextDoneWhen,
    amendments: [
      ...contract.amendments,
      {
        id: generateContractAmendmentId(),
        at,
        by: input.actorId,
        reason,
        before: snapshotForAmendment(contract),
        after: {
          intent: nextIntent,
          scope: nextScope,
          doneWhen: nextDoneWhen,
        },
      },
    ],
  };
}

export function normalizeScope(scope: ContractScope): ContractScope {
  return {
    filesExpected: dedupe(scope.filesExpected),
    filesForbidden: dedupe(scope.filesForbidden),
    ...(scope.maxFilesTouched !== undefined ? { maxFilesTouched: scope.maxFilesTouched } : {}),
  };
}

export function normalizeAmendedCriteria(
  current: readonly DoneWhenCriterion[],
  next: readonly ContractCriterionDraftInput[],
): readonly DoneWhenCriterion[] {
  return next.map((criterion) => {
    const text = criterion.text.trim();
    const existing = criterion.id
      ? current.find((candidate) => candidate.id === criterion.id)
      : undefined;
    const kind = criterion.kind ?? existing?.kind ?? "manual";

    if (!existing) {
      return {
        id: criterion.id ?? generateDoneWhenId(),
        text,
        kind,
      };
    }

    if (existing.text === text && existing.kind === kind) {
      return existing;
    }

    return {
      id: existing.id,
      text,
      kind,
    };
  });
}

export async function resolveActiveContract(
  contractStore: ContractStorePort,
  ref: string,
): Promise<Contract> {
  const contract = await resolveContractRef(contractStore, ref);
  if (!canAmendContract(contract)) {
    throw new MaestroError(`Contract ${contract.id} cannot be modified from status '${contract.status}'`, [
      "Only locked or amended contracts accept amend/criteria changes",
      `Show the contract: maestro task contract show ${contract.id}`,
    ]);
  }
  return contract;
}

function dedupe(values: readonly string[]): readonly string[] {
  const next = values
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
  return Array.from(new Set(next));
}
