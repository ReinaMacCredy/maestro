import { resolveDefaultBase, resolveHeadSha } from "@/shared/lib/git-base.js";
import { MaestroError } from "@/shared/errors.js";
import type { ContractStoreQueryPort, ContractVersionStorePort } from "@/repo/contract-store.port.js";
import type { RunStateStorePort } from "@/repo/run-state-store.port.js";
import { checkCostBudget } from "@/service/check-cost-budget.js";
import { readCurrentContractWithBackfill, readDraftContract } from "@/service/contract-helpers.js";
import type { EvidenceStorePort } from "@/features/evidence/ports/storage.js";
import type { GitAnchorPort } from "@/repo/git-anchor.port.js";
import type { PolicyServices } from "@/features/policy/services.js";
import type { RiskServices } from "@/features/risk/services.js";
import type { LegacySpecStorePort as SpecStorePort } from "@/shared/domain/legacy-spec/index.js";
import type { VerifyServices } from "../verify/services.js";
import type { Verdict, VerdictSubject } from "../domain/types.js";
import type { VerdictStorePort } from "../ports/storage.js";

export interface RequestVerdictDeps {
  readonly contractVersionStore: ContractVersionStorePort;
  readonly contractStore?: ContractStoreQueryPort;
  readonly runStateStore: RunStateStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly verdictStore: VerdictStorePort;
  readonly specStore?: SpecStorePort;
  readonly getEffectiveRiskPolicy: PolicyServices["getEffectiveRiskPolicy"];
  readonly getEffectiveAutopilotPolicy: PolicyServices["getEffectiveAutopilotPolicy"];
  readonly getEffectiveReleasePolicy: PolicyServices["getEffectiveReleasePolicy"];
  readonly getEffectiveSensitivePathsGlobs: PolicyServices["getEffectiveSensitivePathsGlobs"];
  readonly riskServices: RiskServices;
  readonly runTrustVerifier: VerifyServices["runTrustVerifier"];
  readonly gitAnchor: GitAnchorPort;
  readonly projectRoot: string;
}

export async function requestVerdict(
  args: { readonly taskId: string; readonly base?: string; readonly pr?: number },
  deps: RequestVerdictDeps,
): Promise<Verdict> {
  const { taskId, base } = args;

  const contract = await readCurrentContractWithBackfill(
    deps.contractVersionStore,
    deps.contractStore,
    taskId,
  );
  if (contract === undefined) {
    const draft = await readDraftContract(deps.contractStore, taskId);
    if (draft !== undefined) {
      throw new MaestroError(
        `Contract ${draft.id} for task ${taskId} is in draft status — discard it and re-claim the task`,
        ["Contracts are auto-created when a task is claimed from a spec"],
      );
    }
    throw new MaestroError(`No contract found for task ${taskId}`, [
      "Claim a task created from a spec; the contract is synthesized automatically",
      `Re-claim: maestro task claim ${taskId}`,
    ]);
  }

  // Read current run-state early so cost-budget exhaustion is checked before
  // any expensive git/policy operations (BLOCK is the first decision step).
  const runState = await deps.runStateStore.read(taskId);
  const costBudgetCheck = checkCostBudget(contract, runState);
  const costBudgetExhausted = costBudgetCheck.exhausted;

  // Prefer the contract's lock-commit (claimedAtCommit) over branch heuristics
  // so brownfield repos don't pull pre-existing files into the diff and trigger
  // spurious scope errors that escalate risk class. Fall back to branch
  // heuristics only when the contract was locked before the field existed.
  const hasExplicitBase = typeof base === "string" && base.length > 0;
  const resolveBase = (): Promise<string> => {
    if (hasExplicitBase) return Promise.resolve(base as string);
    if (contract.claimedAtCommit) return Promise.resolve(contract.claimedAtCommit);
    return resolveDefaultBase();
  };
  const [baseRef, headSha] = await Promise.all([resolveBase(), resolveHeadSha()]);

  // Anchor git operations to the resolved project root, not runtime cwd. The
  // MCP transport is launched by agent runtimes from their own working
  // directory and scoped via MAESTRO_PROJECT_ROOT, so cwd may diverge from
  // the project being verified.
  const repoRoot = deps.projectRoot;

  const [changedPaths, addedLines, evidenceRows, riskPolicy, autopilotPolicy, releasePolicy, sensitivePathsPolicy, spec] =
    await Promise.all([
      deps.gitAnchor.collectChangedPaths(repoRoot, baseRef, headSha),
      deps.gitAnchor.collectAddedLines(repoRoot, baseRef, headSha),
      deps.evidenceStore.list({ task_id: taskId }),
      deps.getEffectiveRiskPolicy(),
      deps.getEffectiveAutopilotPolicy(),
      deps.getEffectiveReleasePolicy(),
      deps.getEffectiveSensitivePathsGlobs(),
      contract.missionId !== undefined && deps.specStore !== undefined
        ? deps.specStore.read(contract.missionId)
        : Promise.resolve(undefined),
    ]);

  const verifierResult = await deps.runTrustVerifier({
    contract,
    diff: { changedPaths, addedLines, base: baseRef, head: headSha },
  });

  const derivedRiskResult = deps.riskServices.deriveRiskClassFromDiff(
    { changedPaths, sensitivePathsPolicy },
    riskPolicy,
  );

  const rawVerdict = deps.riskServices.computeRisk({
    contract,
    trustFindings: verifierResult.findings,
    evidenceRows: evidenceRows as Parameters<RiskServices["computeRisk"]>[0]["evidenceRows"],
    riskPolicy,
    autopilotPolicy,
    releasePolicy,
    derivedRiskClass: derivedRiskResult.class,
    amendmentCount: contract.amendments.length,
    costBudgetExhausted,
    ...(costBudgetCheck.reason !== undefined ? { costBudgetReason: costBudgetCheck.reason } : {}),
    matchedRiskPolicySignal: derivedRiskResult.matchedRow.signal,
    spec,
  });

  // Stamp subject with tree SHA so verdicts are bound to diff content,
  // not a specific commit hash. Squash survives; force-push invalidates.
  const treeSha = await deps.gitAnchor.resolveTreeSha(repoRoot);
  const subject: VerdictSubject = { tree_sha: treeSha, ...(args.pr !== undefined ? { pr: args.pr } : {}) };
  const verdict: Verdict = { ...rawVerdict, subject };

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
