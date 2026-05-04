import { join } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import { parseYaml } from "@/shared/lib/yaml.js";
import { execArgv } from "@/shared/lib/shell.js";
import type { ContractVersionStorePort } from "@/features/task/ports/contract-version-store.port.js";
import type { EvidenceStorePort } from "@/features/evidence/ports/storage.js";
import type { GitAnchorPort } from "@/features/task/ports/git-anchor.port.js";
import type { PolicyServices } from "@/features/policy/services.js";
import type { RiskServices } from "@/features/risk/services.js";
import type { VerifyServices } from "@/features/verify/services.js";
import type { Verdict } from "../domain/types.js";
import type { VerdictStorePort } from "../ports/storage.js";

export interface RequestVerdictDeps {
  readonly contractVersionStore: ContractVersionStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly verdictStore: VerdictStorePort;
  /** Full policy services — effective getters used when available, raw as fallback. */
  readonly getRiskPolicy: PolicyServices["getRiskPolicy"];
  readonly getAutopilotPolicy: PolicyServices["getAutopilotPolicy"];
  readonly getReleasePolicy: PolicyServices["getReleasePolicy"];
  /** Prefer effective getters (L3.6+); undefined falls back to raw loaders above. */
  readonly getEffectiveRiskPolicy?: PolicyServices["getEffectiveRiskPolicy"];
  readonly getEffectiveAutopilotPolicy?: PolicyServices["getEffectiveAutopilotPolicy"];
  readonly getEffectiveReleasePolicy?: PolicyServices["getEffectiveReleasePolicy"];
  readonly riskServices: RiskServices;
  readonly runTrustVerifier: VerifyServices["runTrustVerifier"];
  readonly gitAnchor: GitAnchorPort;
  readonly projectRoot: string;
}

/**
 * Orchestrates contract + trust-verifier + evidence + policies + risk deriver
 * into a single Verdict, then persists it.
 *
 * L3.6's effective-policy getters are used when present; raw loaders are the
 * fallback for compatibility with branches that haven't merged L3.6 yet.
 */
export async function requestVerdict(
  args: { readonly taskId: string; readonly base?: string },
  deps: RequestVerdictDeps,
): Promise<Verdict> {
  const { taskId, base } = args;

  // 1. Load latest contract
  const contract = await deps.contractVersionStore.readCurrent(taskId);
  if (contract === undefined) {
    throw new Error(`No contract found for task ${taskId}. Run 'maestro contract amend' first.`);
  }

  // 2. Resolve base ref and HEAD sha
  const baseRef = typeof base === "string" && base.length > 0
    ? base
    : await resolveDefaultBase();

  const headResult = await execArgv(["git", "rev-parse", "HEAD"]);
  const headSha = headResult.exitCode === 0 && headResult.stdout
    ? headResult.stdout
    : "HEAD";

  const cwd = process.cwd();

  // 3. Collect diff
  const [changedPaths, addedLines] = await Promise.all([
    deps.gitAnchor.collectChangedPaths(cwd, baseRef, headSha),
    deps.gitAnchor.collectAddedLines(cwd, baseRef, headSha),
  ]);

  // 4. Run trust verifier
  const verifierResult = await deps.runTrustVerifier({
    contract,
    diff: { changedPaths, addedLines, base: baseRef, head: headSha },
  });

  // 5. Load evidence
  const evidenceRows = await deps.evidenceStore.list({ task_id: taskId });

  // 6. Load policies — prefer effective getters (L3.6+) else raw loaders
  const [riskPolicy, autopilotPolicy, releasePolicy] = await Promise.all([
    deps.getEffectiveRiskPolicy !== undefined
      ? deps.getEffectiveRiskPolicy()
      : deps.getRiskPolicy(),
    deps.getEffectiveAutopilotPolicy !== undefined
      ? deps.getEffectiveAutopilotPolicy()
      : deps.getAutopilotPolicy(),
    deps.getEffectiveReleasePolicy !== undefined
      ? deps.getEffectiveReleasePolicy()
      : deps.getReleasePolicy(),
  ]);

  // 7. Load sensitive-paths globs for risk derivation
  const sensitivePathsPolicy = await loadSensitivePathsGlobs(deps.projectRoot);

  // 8. Derive risk class from diff
  const derivedRiskResult = deps.riskServices.deriveRiskClassFromDiff(
    {
      changedPaths,
      addedLines: addedLines.map((line) => ({ path: "", lines: [line] })),
      sensitivePathsPolicy,
    },
    riskPolicy,
  );

  // 9. Count amendments used
  const amendmentCount = contract.amendments.length;

  // 10. Compute verdict
  const verdict = deps.riskServices.computeRisk({
    contract,
    trustFindings: verifierResult.findings,
    evidenceRows: evidenceRows as Parameters<RiskServices["computeRisk"]>[0]["evidenceRows"],
    riskPolicy,
    autopilotPolicy,
    releasePolicy,
    derivedRiskClass: derivedRiskResult.class,
    amendmentCount,
  });

  // 11. Persist verdict
  await deps.verdictStore.write(taskId, verdict);

  return verdict;
}

// ─── helpers ─────────────────────────────────────────────────────────────────

async function resolveDefaultBase(): Promise<string> {
  const upstream = await execArgv(["git", "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
  if (upstream.exitCode === 0 && upstream.stdout) {
    const upstreamRef = upstream.stdout;
    const mergeBase = await execArgv(["git", "merge-base", "HEAD", upstreamRef]);
    if (mergeBase.exitCode === 0 && mergeBase.stdout) {
      return mergeBase.stdout;
    }
    return upstreamRef;
  }

  const mergeBaseMain = await execArgv(["git", "merge-base", "HEAD", "main"]);
  if (mergeBaseMain.exitCode === 0 && mergeBaseMain.stdout) {
    return mergeBaseMain.stdout;
  }

  return "main";
}

interface SensitivePathsYaml {
  readonly paths?: unknown;
}

async function loadSensitivePathsGlobs(projectRoot: string): Promise<readonly string[]> {
  const policyPath = join(projectRoot, ".maestro", "policies", "sensitive-paths.yaml");
  const raw = await readText(policyPath);
  if (raw === undefined) return [];
  try {
    const parsed = parseYaml<SensitivePathsYaml>(raw);
    return Array.isArray(parsed?.paths) ? parsed.paths as string[] : [];
  } catch {
    return [];
  }
}
