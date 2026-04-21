import { MaestroError } from "@/shared/errors.js";
import { canEditContract } from "../../domain/contract/contract-state.js";
import type {
  Contract,
  ContractScope,
  DoneWhenCriterion,
} from "../../domain/contract/contract-types.js";
import type { ContractStorePort } from "../../ports/contract-store.port.js";
import { normalizeAmendedCriteria, normalizeScope } from "./amend-contract.usecase.js";
import { resolveContractRef } from "./resolve-contract.usecase.js";

export interface EditContractInput {
  readonly ref: string;
  readonly intent: string;
  readonly scope: ContractScope;
  readonly doneWhen: readonly Array<{
    readonly id?: string;
    readonly text: string;
    readonly kind?: DoneWhenCriterion["kind"];
  }>;
}

export async function editContract(
  contractStore: ContractStorePort,
  input: EditContractInput,
): Promise<Contract> {
  const contract = await resolveContractRef(contractStore, input.ref);
  if (!canEditContract(contract)) {
    throw new MaestroError(`Contract ${contract.id} cannot be edited from status '${contract.status}'`, [
      "Only draft contracts can be edited directly",
      `Use 'maestro task contract amend ${contract.id} --reason \"...\"' once the contract is locked`,
    ]);
  }

  return contractStore.save({
    ...contract,
    intent: input.intent.trim(),
    scope: normalizeScope(input.scope),
    doneWhen: normalizeAmendedCriteria(contract.doneWhen, input.doneWhen),
  });
}
