import type { CheckRunConclusion, GithubApiPort } from "../ports/github-api.port.js";
import type { Verdict, VerdictDecision } from "@/features/verdict/domain/types.js";
import type { VerdictOverridePayload } from "@/features/evidence/index.js";
import { assertNever } from "@/shared/lib/assert-never.js";

export interface PostPrCheckDeps {
  readonly githubApi: GithubApiPort;
}

export interface PostPrCheckArgs {
  readonly verdict: Verdict;
  readonly repository: string;
  readonly headSha: string;
  readonly existingCheckRunId?: number;
  /** Override rows for this verdict, if any. Appended to summary — does NOT change conclusion. */
  readonly overrides?: readonly VerdictOverridePayload[];
  /**
   * When set, overrides the conclusion to `failure` regardless of verdict decision and appends
   * this message to the summary. Used by the deploy-authorization gate (L7.9).
   */
  readonly deployBlockReason?: string;
}

function conclusionFor(decision: VerdictDecision): CheckRunConclusion {
  switch (decision) {
    case "PASS":  return "success";
    case "FAIL":  return "failure";
    case "BLOCK": return "failure";
    case "HUMAN": return "action_required";
    default: return assertNever(decision);
  }
}

function buildSummary(
  verdict: Verdict,
  overrides?: readonly VerdictOverridePayload[],
): string {
  const riskLine = `Effective risk class: ${verdict.effectiveRiskClass}.`;
  const reasonPart = verdict.reasons.length === 0
    ? riskLine
    : `${riskLine}\n${verdict.reasons.map((r) => `- ${r.message}`).join("\n")}`;

  if (overrides === undefined || overrides.length === 0) {
    return reasonPart;
  }

  const overrideLines = overrides
    .map((ov) => `Verdict overridden by ${ov.overriddenBy}: ${ov.reason}`)
    .join("\n");
  return `${reasonPart}\n${overrideLines}`;
}

export async function postPrCheck(
  args: PostPrCheckArgs,
  deps: PostPrCheckDeps,
): Promise<void> {
  const { verdict, repository, headSha, existingCheckRunId, overrides, deployBlockReason } = args;

  const baseConclusion = conclusionFor(verdict.decision);
  const conclusion: CheckRunConclusion = deployBlockReason !== undefined ? "failure" : baseConclusion;
  const baseSummary = buildSummary(verdict, overrides);
  const summary = deployBlockReason !== undefined
    ? `${baseSummary}\n\n${deployBlockReason}`
    : baseSummary;

  const input = {
    repository,
    headSha,
    name: "Maestro Verify",
    conclusion,
    title: `Maestro Verdict: ${verdict.decision}`,
    summary,
  };

  if (existingCheckRunId !== undefined) {
    await deps.githubApi.patchCheckRun({ ...input, checkRunId: existingCheckRunId });
    return;
  }

  await deps.githubApi.postCheckRun(input);
}
