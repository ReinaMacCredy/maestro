import { MaestroError } from "@/shared/errors.js";
import { recordEvidence } from "@/features/evidence/index.js";
import type { EvidenceStorePort } from "@/features/evidence/index.js";
import type { ContractAmendment } from "../domain/contract/contract-types.js";
import type { ContractVersionStorePort } from "../ports/contract-version-store.port.js";

export interface AmendContractInput {
  readonly taskId: string;
  readonly amendment: ContractAmendment;
  /** paths added to filesExpected by this amendment */
  readonly addedPaths: readonly string[];
  /** paths removed from filesExpected by this amendment */
  readonly removedPaths: readonly string[];
}

export async function amendContract(
  store: ContractVersionStorePort,
  evidenceStore: EvidenceStorePort,
  input: AmendContractInput,
): Promise<void> {
  const current = await store.readCurrent(input.taskId);
  if (current === undefined) {
    throw new MaestroError(
      `No contract found for task ${input.taskId}`,
      ["Propose a contract before amending it"],
    );
  }

  const { amendmentBudget } = current;
  if (amendmentBudget !== undefined) {
    // Rule: total amendments after this one must be <= maxAmendments
    const existingCount = current.amendments.length;
    if (existingCount >= amendmentBudget.maxAmendments) {
      await recordEvidence(evidenceStore, {
        task_id: input.taskId,
        kind: "contract-amendment-blocked",
        witness_level: "witnessed-by-maestro",
        payload: {
          reason: "budget_exhausted",
          attemptedPaths: input.addedPaths as string[],
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

    // Rule: number of added paths must be <= maxPathsPerAmendment
    if (input.addedPaths.length > amendmentBudget.maxPathsPerAmendment) {
      await recordEvidence(evidenceStore, {
        task_id: input.taskId,
        kind: "contract-amendment-blocked",
        witness_level: "witnessed-by-maestro",
        payload: {
          reason: "budget_exhausted",
          attemptedPaths: input.addedPaths as string[],
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

    // Rule: added paths must not match any forbidden pattern
    if (amendmentBudget.forbiddenAmendmentPaths.length > 0) {
      const forbidden = await findForbiddenPathMatches(
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
            attemptedPaths: input.addedPaths as string[],
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

  // Success: write new version with the amendment appended
  const versions = await store.history(input.taskId);
  const nextVersion = versions.length + 1;
  await store.write(input.taskId, nextVersion, {
    ...current,
    amendments: [...current.amendments, input.amendment],
    status: "amended",
  });

  await recordEvidence(evidenceStore, {
    task_id: input.taskId,
    kind: "contract-amendment",
    witness_level: "witnessed-by-maestro",
    payload: {
      amendmentId: input.amendment.id,
      addedPaths: input.addedPaths as string[],
      removedPaths: input.removedPaths as string[],
      reason: input.amendment.reason,
    },
  });
}

async function findForbiddenPathMatches(
  paths: readonly string[],
  forbiddenPatterns: readonly string[],
): Promise<string[]> {
  const matched: string[] = [];
  for (const path of paths) {
    for (const pattern of forbiddenPatterns) {
      const glob = new Bun.Glob(pattern);
      if (glob.match(path)) {
        matched.push(path);
        break;
      }
    }
  }
  return matched;
}
