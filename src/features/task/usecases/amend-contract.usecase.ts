import { MaestroError } from "@/shared/errors.js";
import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import { recordEvidence } from "@/features/evidence/index.js";
import type { EvidenceStorePort } from "@/features/evidence/index.js";
import type { ContractAmendment } from "../domain/contract/contract-types.js";
import type { ContractStoreQueryPort } from "../ports/contract-store.port.js";
import type { ContractVersionStorePort } from "../ports/contract-version-store.port.js";
import {
  readContractHistoryWithBackfill,
  readCurrentContractWithBackfill,
} from "./read-current-contract-with-backfill.js";

export interface AmendContractInput {
  readonly taskId: string;
  readonly amendment: ContractAmendment;
  /** paths added to filesExpected by this amendment */
  readonly addedPaths: readonly string[];
  /** paths removed from filesExpected by this amendment */
  readonly removedPaths: readonly string[];
}

export interface AmendContractResult {
  readonly newVersion: number;
  readonly amendmentId: string;
}

// Three-arg overload preserves the pre-bridge call signature for tests that
// build the v2 store directly. Production callers pass a legacyStore so
// pre-bridge L1-only contracts get backfilled on first read.
export function amendContract(
  store: ContractVersionStorePort,
  evidenceStore: EvidenceStorePort,
  input: AmendContractInput,
): Promise<AmendContractResult>;
export function amendContract(
  store: ContractVersionStorePort,
  legacyStore: ContractStoreQueryPort,
  evidenceStore: EvidenceStorePort,
  input: AmendContractInput,
): Promise<AmendContractResult>;
export async function amendContract(
  store: ContractVersionStorePort,
  legacyOrEvidence: ContractStoreQueryPort | EvidenceStorePort,
  evidenceOrInput: EvidenceStorePort | AmendContractInput,
  maybeInput?: AmendContractInput,
): Promise<AmendContractResult> {
  const { legacyStore, evidenceStore, input } = maybeInput === undefined
    ? {
        legacyStore: undefined,
        evidenceStore: legacyOrEvidence as EvidenceStorePort,
        input: evidenceOrInput as AmendContractInput,
      }
    : {
        legacyStore: legacyOrEvidence as ContractStoreQueryPort,
        evidenceStore: evidenceOrInput as EvidenceStorePort,
        input: maybeInput,
      };
  const current = await readCurrentContractWithBackfill(store, legacyStore, input.taskId);
  if (current === undefined) {
    throw new MaestroError(
      `No contract found for task ${input.taskId}`,
      ["Propose a contract before amending it"],
    );
  }

  const { amendmentBudget } = current;
  if (amendmentBudget !== undefined) {
    const existingCount = current.amendments.length;
    if (existingCount >= amendmentBudget.maxAmendments) {
      await recordEvidence(evidenceStore, {
        task_id: input.taskId,
        kind: "contract-amendment-blocked",
        witness_level: "witnessed-by-maestro",
        payload: {
          reason: "budget_exhausted",
          attemptedPaths: input.addedPaths,
          details: `Amendment budget exhausted: ${existingCount} of ${amendmentBudget.maxAmendments} amendments already used`,
        },
      });
      throw new MaestroError(
        `Amendment budget exhausted for task ${input.taskId}: ${existingCount} of ${amendmentBudget.maxAmendments} amendments used`,
        [
          "Increase amendmentBudget.maxAmendments on the contract or work within the existing scope",
        ],
      );
    }

    if (input.addedPaths.length > amendmentBudget.maxPathsPerAmendment) {
      await recordEvidence(evidenceStore, {
        task_id: input.taskId,
        kind: "contract-amendment-blocked",
        witness_level: "witnessed-by-maestro",
        payload: {
          reason: "budget_exhausted",
          attemptedPaths: input.addedPaths,
          details: `Too many added paths: ${input.addedPaths.length} exceeds maxPathsPerAmendment (${amendmentBudget.maxPathsPerAmendment})`,
        },
      });
      throw new MaestroError(
        `Amendment adds too many paths for task ${input.taskId}: ${input.addedPaths.length} exceeds maxPathsPerAmendment (${amendmentBudget.maxPathsPerAmendment})`,
        [
          "Split the amendment into smaller chunks or increase amendmentBudget.maxPathsPerAmendment",
        ],
      );
    }

    if (amendmentBudget.forbiddenAmendmentPaths.length > 0) {
      const forbidden = findForbiddenPathMatches(
        input.addedPaths,
        amendmentBudget.forbiddenAmendmentPaths,
      );
      if (forbidden.length > 0) {
        await recordEvidence(evidenceStore, {
          task_id: input.taskId,
          kind: "contract-amendment-blocked",
          witness_level: "witnessed-by-maestro",
          payload: {
            reason: "forbidden_path",
            attemptedPaths: input.addedPaths,
            details: `Added paths match forbidden patterns: ${forbidden.join(", ")}`,
          },
        });
        throw new MaestroError(
          `Amendment for task ${input.taskId} includes forbidden paths: ${forbidden.join(", ")}`,
          [
            "Remove the forbidden paths from the amendment",
            `Forbidden patterns: ${amendmentBudget.forbiddenAmendmentPaths.join(", ")}`,
          ],
        );
      }
    }
  }

  const versions = await readContractHistoryWithBackfill(store, legacyStore, input.taskId);
  const nextVersion = versions.length + 1;
  // Apply the amendment's after.snapshot to the contract's effective state.
  // Without this, the new version still carries the pre-amendment scope and
  // every downstream reader (Trust Verifier scope check, plan check) sees
  // the un-amended contract — agents amend, the verifier still complains,
  // and they conclude amend is broken.
  const afterScope = input.amendment.after.scope ?? current.scope;
  const afterIntent = input.amendment.after.intent ?? current.intent;
  const afterDoneWhen = input.amendment.after.doneWhen ?? current.doneWhen;
  await store.write(input.taskId, nextVersion, {
    ...current,
    intent: afterIntent,
    scope: afterScope,
    doneWhen: afterDoneWhen,
    amendments: [...current.amendments, input.amendment],
    status: "amended",
  });

  await recordEvidence(evidenceStore, {
    task_id: input.taskId,
    kind: "contract-amendment",
    witness_level: "witnessed-by-maestro",
    payload: {
      amendmentId: input.amendment.id,
      addedPaths: input.addedPaths,
      removedPaths: input.removedPaths,
      reason: input.amendment.reason,
    },
  });

  return { newVersion: nextVersion, amendmentId: input.amendment.id };
}

function findForbiddenPathMatches(
  paths: readonly string[],
  forbiddenPatterns: readonly string[],
): string[] {
  const matched: string[] = [];
  for (const path of paths) {
    if (matchesAnyGlob(forbiddenPatterns, path)) {
      matched.push(path);
    }
  }
  return matched;
}
