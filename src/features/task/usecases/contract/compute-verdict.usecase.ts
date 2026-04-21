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

  const candidates = (await contractStore.all()).filter((candidate) =>
    candidate.id !== contract.id
    && candidate.repoRoot === contract.repoRoot
    && candidate.status !== "draft"
    && candidate.status !== "discarded",
  );
  if (candidates.length === 0) {
    return undefined;
  }

  const results = await Promise.all(candidates.map(async (candidate) => {
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
    return overlaps ? candidate.id : undefined;
  }));
  const overlapping = results.filter((id): id is string => id !== undefined).sort();

  if (overlapping.length === 0) {
    return undefined;
  }

  return {
    otherContractIds: overlapping,
    policy: contract.configSnapshot.overlapPolicy,
  };
}
