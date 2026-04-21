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
  const overlapDetected = await detectContractOverlap(contractStore, contract, at);
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
  contract: Contract,
  at: string,
): Promise<ContractVerdict["overlapDetected"] | undefined> {
  const currentWindow = resolveContractWindow(contract, at);
  if (!currentWindow) {
    return undefined;
  }

  const overlapping = (await contractStore.all())
    .filter((candidate) => candidate.id !== contract.id && candidate.repoRoot === contract.repoRoot)
    .filter((candidate) => candidate.status !== "draft" && candidate.status !== "discarded")
    .filter((candidate) => {
      const candidateWindow = resolveContractWindow(candidate, at);
      return candidateWindow !== undefined && windowsOverlap(currentWindow, candidateWindow);
    })
    .map((candidate) => candidate.id)
    .sort();

  if (overlapping.length === 0) {
    return undefined;
  }

  return {
    otherContractIds: overlapping,
    policy: contract.configSnapshot.overlapPolicy,
  };
}

function resolveContractWindow(
  contract: Contract,
  fallbackEndAt: string,
): { readonly startAt: number; readonly endAt: number } | undefined {
  const startAt = Date.parse(contract.lockedAt ?? contract.createdAt);
  const endAt = Date.parse(contract.closedAt ?? fallbackEndAt);
  if (!Number.isFinite(startAt) || !Number.isFinite(endAt)) {
    return undefined;
  }
  return {
    startAt,
    endAt,
  };
}

function windowsOverlap(
  left: { readonly startAt: number; readonly endAt: number },
  right: { readonly startAt: number; readonly endAt: number },
): boolean {
  return left.startAt <= right.endAt && right.startAt <= left.endAt;
}
