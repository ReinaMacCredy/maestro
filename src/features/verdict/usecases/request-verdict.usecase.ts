import { resolveDefaultBase, resolveHeadSha } from "@/shared/lib/git-base.js";
import type { ContractVersionStorePort } from "@/features/task/ports/contract-version-store.port.js";
import type { RunStateStorePort } from "@/features/task/ports/run-state-store.port.js";
import { checkCostBudget } from "@/features/task/index.js";
import type { EvidenceStorePort } from "@/features/evidence/ports/storage.js";
import type { GitAnchorPort } from "@/features/task/ports/git-anchor.port.js";
import type { PolicyServices } from "@/features/policy/services.js";
import { loadSensitivePathsGlobs } from "@/features/policy/index.js";
import type { RiskServices } from "@/features/risk/services.js";
import type { VerifyServices } from "@/features/verify/services.js";
import type { Verdict } from "../domain/types.js";
import type { VerdictStorePort } from "../ports/storage.js";

export interface RequestVerdictDeps {
  readonly contractVersionStore: ContractVersionStorePort;
  readonly runStateStore: RunStateStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly verdictStore: VerdictStorePort;
  readonly getEffectiveRiskPolicy: PolicyServices["getEffectiveRiskPolicy"];
  readonly getEffectiveAutopilotPolicy: PolicyServices["getEffectiveAutopilotPolicy"];
  readonly getEffectiveReleasePolicy: PolicyServices["getEffectiveReleasePolicy"];
  readonly riskServices: RiskServices;
  readonly runTrustVerifier: VerifyServices["runTrustVerifier"];
  readonly gitAnchor: GitAnchorPort;
  readonly projectRoot: string;
}

export async function requestVerdict(
  args: { readonly taskId: string; readonly base?: string },
  deps: RequestVerdictDeps,
): Promise<Verdict> {
  const { taskId, base } = args;

  const contract = await deps.contractVersionStore.readCurrent(taskId);
  if (contract === undefined) {
    throw new Error(`No contract found for task ${taskId}. Run 'maestro contract amend' first.`);
  }

  // Read current run-state early so cost-budget exhaustion is checked before
  // any expensive git/policy operations (BLOCK is the first decision step).
  const runState = await deps.runStateStore.read(taskId);
  const costBudgetExhausted = checkCostBudget(contract, runState).exhausted;

  const baseRef = typeof base === "string" && base.length > 0
    ? base
    : await resolveDefaultBase();
  const headSha = await resolveHeadSha();

  const cwd = process.cwd();

  const [changedPaths, addedLines, evidenceRows, riskPolicy, autopilotPolicy, releasePolicy, sensitivePathsPolicy] =
    await Promise.all([
      deps.gitAnchor.collectChangedPaths(cwd, baseRef, headSha),
      deps.gitAnchor.collectAddedLines(cwd, baseRef, headSha),
      deps.evidenceStore.list({ task_id: taskId }),
      deps.getEffectiveRiskPolicy(),
      deps.getEffectiveAutopilotPolicy(),
      deps.getEffectiveReleasePolicy(),
      loadSensitivePathsGlobs(deps.projectRoot),
    ]);

  const verifierResult = await deps.runTrustVerifier({
    contract,
    diff: { changedPaths, addedLines, base: baseRef, head: headSha },
  });

  const derivedRiskResult = deps.riskServices.deriveRiskClassFromDiff(
    { changedPaths, sensitivePathsPolicy },
    riskPolicy,
  );

  const verdict = deps.riskServices.computeRisk({
    contract,
    trustFindings: verifierResult.findings,
    evidenceRows: evidenceRows as Parameters<RiskServices["computeRisk"]>[0]["evidenceRows"],
    riskPolicy,
    autopilotPolicy,
    releasePolicy,
    derivedRiskClass: derivedRiskResult.class,
    amendmentCount: contract.amendments.length,
    costBudgetExhausted,
  });

  await deps.verdictStore.write(taskId, verdict);

  // Increment retryCount on non-terminal decisions so the cost-budget gate
  // can BLOCK on the next attempt when maxRetries is reached.
  // PASS is terminal (done); BLOCK means already exhausted — do not double-count.
  // Failure here is non-fatal: verdict is already persisted and append-only.
  if (verdict.decision === "FAIL" || verdict.decision === "HUMAN") {
    try {
      await deps.runStateStore.increment(taskId, { retryCount: 1 });
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      process.stderr.write(`[warn] failed to increment run-state retryCount for ${taskId}: ${msg}\n`);
    }
  }

  return verdict;
}
