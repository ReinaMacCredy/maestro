import { applyPathChanges } from "./contract-helpers.js";
import { amendContract } from "@/shared/domain/legacy-task/usecases/amend-contract.usecase.js";
import { getCurrentContract } from "@/shared/domain/legacy-task/usecases/get-current-contract.usecase.js";
import { generateContractAmendmentId } from "@/shared/domain/legacy-task/domain/contract/contract-state.js";
import type { ContractAmendment } from "@/types/contract.js";
import type { Services } from "@/services.js";

export type AmendContractScopeResult =
  | { readonly kind: "no-contract" }
  | { readonly kind: "no-changes" }
  | {
      readonly kind: "amended";
      readonly newVersion: number;
      readonly amendmentId: string;
      readonly skippedAddPaths: readonly string[];
    };

export interface AmendContractScopeInput {
  readonly taskId: string;
  readonly addPaths: readonly string[];
  readonly removePaths: readonly string[];
  readonly reason: string;
  readonly by: string;
}

export async function amendContractScope(
  services: Pick<
    Services,
    "contractStore" | "contractVersionStore" | "evidenceStore"
  >,
  input: AmendContractScopeInput,
): Promise<AmendContractScopeResult> {
  const before = await getCurrentContract(
    services.contractVersionStore,
    services.contractStore,
    input.taskId,
  );
  if (!before) return { kind: "no-contract" };

  const { result: newFilesExpected, skipped: skippedAddPaths } =
    applyPathChanges(before.scope.filesExpected, input.addPaths, input.removePaths);

  // Compare as sets so simultaneous remove+re-add of the same path
  // (which reorders the array but leaves semantic scope intact)
  // doesn't trigger a spurious amendment.
  const beforeSet = new Set(before.scope.filesExpected);
  const afterSet = new Set(newFilesExpected);
  const scopeChanged =
    beforeSet.size !== afterSet.size ||
    before.scope.filesExpected.some((p) => !afterSet.has(p));
  if (!scopeChanged) return { kind: "no-changes" };

  const amendment: ContractAmendment = {
    id: generateContractAmendmentId(),
    at: new Date().toISOString(),
    by: input.by,
    reason: input.reason,
    before: { scope: before.scope },
    after: {
      scope: {
        filesExpected: newFilesExpected,
        filesForbidden: before.scope.filesForbidden,
      },
    },
  };

  const { newVersion, amendmentId } = await amendContract(
    services.contractVersionStore,
    services.contractStore,
    services.evidenceStore,
    {
      taskId: input.taskId,
      amendment,
      addedPaths: input.addPaths,
      removedPaths: input.removePaths,
    },
  );

  return { kind: "amended", newVersion, amendmentId, skippedAddPaths };
}
