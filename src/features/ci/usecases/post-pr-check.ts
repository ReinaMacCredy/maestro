import type { GithubApiPort } from "../ports/github-api.port.js";
import type { Verdict, VerdictDecision } from "@/features/verdict/domain/types.js";

export interface PostPrCheckDeps {
  readonly githubApi: GithubApiPort;
}

export interface PostPrCheckArgs {
  readonly verdict: Verdict;
  readonly repository: string;
  readonly headSha: string;
  readonly existingCheckRunId?: number;
}

type CheckConclusion = "success" | "failure" | "action_required";

function conclusionFor(decision: VerdictDecision): CheckConclusion {
  switch (decision) {
    case "PASS":  return "success";
    case "FAIL":  return "failure";
    case "BLOCK": return "failure";
    case "HUMAN": return "action_required";
  }
}

function buildSummary(verdict: Verdict): string {
  const riskLine = `Effective risk class: ${verdict.effectiveRiskClass}.`;
  if (verdict.reasons.length === 0) {
    return riskLine;
  }
  const reasonLines = verdict.reasons.map((r) => `- ${r.message}`).join("\n");
  return `${riskLine}\n${reasonLines}`;
}

export async function postPrCheck(
  args: PostPrCheckArgs,
  deps: PostPrCheckDeps,
): Promise<{ checkRunId: number }> {
  const { verdict, repository, headSha, existingCheckRunId } = args;

  const conclusion = conclusionFor(verdict.decision);
  const title = `Maestro Verdict: ${verdict.decision}`;
  const summary = buildSummary(verdict);

  if (existingCheckRunId !== undefined) {
    await deps.githubApi.patchCheckRun({
      repository,
      headSha,
      name: "Maestro Verify",
      conclusion,
      title,
      summary,
      checkRunId: existingCheckRunId,
    });
    return { checkRunId: existingCheckRunId };
  }

  const ref = await deps.githubApi.postCheckRun({
    repository,
    headSha,
    name: "Maestro Verify",
    conclusion,
    title,
    summary,
  });
  return { checkRunId: ref.id };
}
