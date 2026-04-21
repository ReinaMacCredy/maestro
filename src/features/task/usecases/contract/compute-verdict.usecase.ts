import type { Task, TaskReceipt } from "../../domain/task-types.js";
import { computeContractVerdict, type ComputedContractVerdict } from "../../domain/contract/verdict.js";
import type { Contract } from "../../domain/contract/contract-types.js";
import type { ContractVerdict } from "../../domain/contract/contract-types.js";
import type { ContractStoreQueryPort } from "../../ports/contract-store.port.js";
import type { GitAnchorPort } from "../../ports/git-anchor.port.js";

export async function computeContractVerdictForTask(
  contractStore: ContractStoreQueryPort,
  gitAnchor: GitAnchorPort,
  contract: Contract,
  task: Pick<Task, "assignee" | "receipt" | "updatedAt">,
  receiptOverride?: TaskReceipt,
): Promise<ComputedContractVerdict & { readonly closedAtCommit?: string }> {
  const gitResult = await gitAnchor.collectTouchedFiles({
    repoRoot: contract.repoRoot,
    claimedAtCommit: contract.claimedAtCommit,
    rebaseFallback: contract.configSnapshot.rebaseFallback,
  });
  const at = task.updatedAt;
  const actorId = task.assignee ?? contract.lockedBy ?? contract.createdBy;
  const receipt = receiptOverride ?? task.receipt;
  const overlapDetected = await detectContractOverlap(contractStore, gitAnchor, contract, gitResult.closedAtCommit);
  const computed = computeContractVerdict(contract, gitResult, receipt, actorId, at, {
    overlapDetected,
  });

  return {
    ...computed,
    closedAtCommit: gitResult.closedAtCommit,
  };
}

async function detectContractOverlap(
  contractStore: ContractStoreQueryPort,
  gitAnchor: GitAnchorPort,
  contract: Contract,
  currentClosedAtCommit: string | undefined,
): Promise<ContractVerdict["overlapDetected"] | undefined> {
  if (!contract.claimedAtCommit || !currentClosedAtCommit) {
    return undefined;
  }

  const overlapping: string[] = [];
  for (const candidate of await contractStore.all()) {
    if (candidate.id === contract.id || candidate.repoRoot !== contract.repoRoot) {
      continue;
    }
    if (candidate.status === "draft" || candidate.status === "discarded") {
      continue;
    }

    const overlaps = await gitAnchor.windowsOverlap({
      repoRoot: contract.repoRoot,
      left: {
        claimedAtCommit: contract.claimedAtCommit,
        closedAtCommit: currentClosedAtCommit,
      },
      right: {
        claimedAtCommit: candidate.claimedAtCommit,
        closedAtCommit: candidate.closedAtCommit ?? currentClosedAtCommit,
      },
    });
    if (overlaps) {
      overlapping.push(candidate.id);
    }
  }
  overlapping.sort();

  if (overlapping.length === 0) {
    return undefined;
  }

  return {
    otherContractIds: overlapping,
    policy: contract.configSnapshot.overlapPolicy,
  };
}
