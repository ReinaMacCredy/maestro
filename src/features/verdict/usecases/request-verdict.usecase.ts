import { resolveDefaultBase, resolveHeadSha } from "@/shared/lib/git-base.js";
import type { ContractVersionStorePort } from "@/features/task/ports/contract-version-store.port.js";
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
  });

  await deps.verdictStore.write(taskId, verdict);

  return verdict;
}
