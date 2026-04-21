import type { Task, TaskReceipt } from "../../domain/task-types.js";
import { computeContractVerdict, type ComputedContractVerdict } from "../../domain/contract/verdict.js";
import type { Contract } from "../../domain/contract/contract-types.js";
import type { GitAnchorPort } from "../../ports/git-anchor.port.js";

export async function computeContractVerdictForTask(
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
  const computed = computeContractVerdict(contract, gitResult, receipt, actorId, at);

  return {
    ...computed,
    closedAtCommit: gitResult.closedAtCommit,
  };
}
