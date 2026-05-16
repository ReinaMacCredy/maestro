import { appendFile } from "node:fs/promises";
import { readJson } from "@/shared/lib/fs.js";
import { mapWithConcurrency } from "@/shared/lib/concurrency.js";
import type { EvidenceStorePort } from "@/features/evidence/ports/storage.js";
import { recordEvidence } from "@/features/evidence/index.js";
import type {
  CommandPayload,
  CrossTaskConflictPayload,
  VerdictOverridePayload,
  DeployReadinessPayload,
} from "@/features/evidence/domain/types.js";
import { detectCrossTaskConflict } from "./detect-cross-task-conflict.js";
import { requestVerdict } from "@/features/verdict/index.js";
import type { RequestVerdictDeps } from "@/features/verdict/index.js";
import type { Verdict } from "@/features/verdict/domain/types.js";
import { checkArchitectureRules } from "@/shared/lib/arch-rules.js";
import type { LintViolationPayload } from "@/features/evidence/domain/types.js";
import { loadOwnersFromBase } from "@/features/policy/index.js";
import type { GithubApiPort } from "../ports/github-api.port.js";
import type { CiEnv } from "../domain/ci-env.js";
import { postPrCheck } from "./post-pr-check.js";
import type { PostPrCheckDeps } from "./post-pr-check.js";

export interface TestResultPayload {
  readonly passed: number;
  readonly failed: number;
  readonly skipped?: number;
  readonly total?: number;
  readonly duration_ms?: number;
  readonly suite?: string;
}

export interface RunCiVerifyDeps {
  readonly env: CiEnv;
  readonly evidenceStore: EvidenceStorePort;
  readonly verdict: { readonly request: typeof requestVerdict };
  readonly verdictDeps: RequestVerdictDeps;
  readonly prCheck?: PostPrCheckDeps;
  readonly githubApi?: GithubApiPort;
  readonly projectRoot?: string;
  /** Overridable for testing — defaults to reading owners.yaml from the base ref via git. */
  readonly loadOwnersFromBase?: (
    base: string,
    projectRoot: string,
  ) => Promise<import("@/features/policy/index.js").Owners>
    | import("@/features/policy/index.js").Owners;
  readonly readTestResults?: (path: string) => Promise<TestResultPayload | undefined>;
  readonly writeOutput?: (key: string, value: string) => Promise<void>;
  readonly now?: () => Date;
}

export interface RunCiVerifyArgs {
  readonly taskId: string;
  readonly pr?: number;
  readonly base?: string;
  readonly testResultsPath?: string;
}

export async function runCiVerify(
  args: RunCiVerifyArgs,
  deps: RunCiVerifyDeps,
): Promise<Verdict> {
  const { taskId } = args;

  const resolvedBase = args.base ?? deps.env.baseRef;

  const testResultsPath = args.testResultsPath ?? deps.env.testResultsFile;
  if (typeof testResultsPath === "string" && testResultsPath.length > 0) {
    const readTestResults = deps.readTestResults ?? defaultReadTestResults;
    try {
      const results = await readTestResults(testResultsPath);
      if (results !== undefined) {
        const payload: CommandPayload = {
          command: `ci-test-results:${testResultsPath}`,
          exit: results.failed > 0 ? 1 : 0,
          duration_ms: results.duration_ms,
        };
        await recordEvidence(deps.evidenceStore, {
          task_id: taskId,
          kind: "command",
          payload,
          witness_level: "witnessed-by-ci",
        });
      }
    } catch {
      // test results ingestion failure is non-fatal; continue to verdict
    }
  }

  const resolvedPr = args.pr ?? deps.env.pr;

  // ─── Cross-task conflict detection ────────────────────────────────────
  // Run BEFORE verdict.request so the evidence row is present when Risk Engine reads it.
  if (
    deps.githubApi !== undefined &&
    deps.env.repository !== undefined &&
    resolvedPr !== undefined
  ) {
    const githubApi = deps.githubApi;
    const repository = deps.env.repository;
    try {
      const otherPrs = (await githubApi.listOpenPullRequests({ repository })).filter(
        (pr) => pr !== resolvedPr,
      );
      // GitHub's secondary rate limit (CONTENTION_REQUEST_LIMITS) trips around
      // 10 concurrent requests against the same repository. Cap parallelism
      // well below that for a generous safety margin on busy repos.
      const FILES_CONCURRENCY = 4;
      const [thisPrFiles, otherPrFiles] = await Promise.all([
        githubApi.getPullRequestFiles({ repository, pr: resolvedPr }),
        mapWithConcurrency(otherPrs, FILES_CONCURRENCY, async (pr): Promise<{ pr: number; files: readonly string[] }> => ({
          pr,
          files: await githubApi.getPullRequestFiles({ repository, pr }),
        })),
      ]);
      const conflictResult = detectCrossTaskConflict({
        thisPrFiles,
        otherPrs: otherPrFiles,
      });
      if (conflictResult.conflictingPrs.length > 0) {
        const conflictPayload: CrossTaskConflictPayload = {
          thisPr: resolvedPr,
          conflictingPrs: conflictResult.conflictingPrs,
          overlappingPaths: conflictResult.overlappingPaths,
        };
        await recordEvidence(deps.evidenceStore, {
          task_id: taskId,
          kind: "cross-task-conflict",
          payload: conflictPayload,
          witness_level: "witnessed-by-ci",
        });
      }
    } catch {
      // non-fatal — GitHub API failure skips conflict detection silently
    }
  }

  const verdict = await deps.verdict.request(
    { taskId, base: resolvedBase, pr: resolvedPr },
    deps.verdictDeps,
  );

  // Record architecture-lint findings as `lint-violation` evidence at
  // `witnessed-by-ci`, so C-1's `task introspect` can surface "open lints"
  // raised by CI runs. Diff-aware rule (`no-hand-edit-generated`) is enabled
  // when GITHUB_BASE_REF is available; otherwise it self-skips with an info
  // finding (not an error), which we drop below.
  try {
    const projectRoot = deps.projectRoot ?? deps.verdictDeps.projectRoot;
    let lintDiff: { base: string; changedPaths: readonly string[] } | undefined;
    if (resolvedBase) {
      const headCommit = await deps.verdictDeps.gitAnchor.resolveHeadCommit(projectRoot);
      if (headCommit !== undefined) {
        lintDiff = {
          base: resolvedBase,
          changedPaths: await deps.verdictDeps.gitAnchor.collectChangedPaths(
            projectRoot,
            resolvedBase,
            headCommit,
          ),
        };
      }
    }
    const violations = await checkArchitectureRules({
      repoRoot: projectRoot,
      ...(lintDiff ? { diff: lintDiff } : {}),
    });
    await Promise.all(
      violations
        .filter((v) => v.severity === "error" || v.severity === "warn")
        .map((v) => {
          const payload: LintViolationPayload = {
            ruleId: v.ruleId,
            file: v.file,
            ...(v.line !== undefined ? { line: v.line } : {}),
            ...(v.snippet !== undefined ? { snippet: v.snippet } : {}),
            message: v.message,
            remediation: v.remediation,
          };
          return recordEvidence(deps.evidenceStore, {
            task_id: taskId,
            kind: "lint-violation",
            payload,
            witness_level: "witnessed-by-ci",
          });
        }),
    );
  } catch {
    // Non-fatal: lint-violation recording is observational; verdict already
    // reflects the underlying findings via the Trust Verifier.
  }

  const outputPath = deps.env.outputPath;
  if (typeof outputPath === "string" && outputPath.length > 0) {
    const writeOutput = deps.writeOutput ?? makeDefaultWriteOutput(outputPath);
    await writeOutput("verdict_id", verdict.id);
    await writeOutput("verdict_decision", verdict.decision);
    await writeOutput("effective_risk_class", verdict.effectiveRiskClass);
  }

  let deployBlockReason: string | undefined;
  if (
    deps.env.provider === "github-actions" &&
    deps.githubApi !== undefined &&
    deps.env.repository !== undefined &&
    resolvedPr !== undefined
  ) {
    let deployReadiness: { payload: DeployReadinessPayload } | undefined;
    try {
      const rows = await deps.evidenceStore.list({
        task_id: taskId,
        kind: "deploy-readiness",
      });
      deployReadiness = rows.find(
        (r) => (r.payload as DeployReadinessPayload).gate === "pass",
      ) as { payload: DeployReadinessPayload } | undefined;
    } catch {
      // non-fatal — if evidence list fails, skip the deploy gate
    }

    if (deployReadiness !== undefined) {
      try {
        const author = await deps.githubApi.getPullRequestAuthor({
          repository: deps.env.repository,
          pr: resolvedPr,
        });
        // Rule 12: load owners from base, not PR head, so a PR cannot promote itself.
        const loadOwnersFn = deps.loadOwnersFromBase ?? loadOwnersFromBase;
        const owners = await loadOwnersFn(resolvedBase ?? "main", deps.projectRoot ?? process.cwd());
        if (!owners.deployApprovers.includes(author)) {
          deployBlockReason = `deploy not authorized: PR author \`${author}\` is not in owners.yaml deploy_approver`;
        }
      } catch {
        // non-fatal — if we can't resolve author or owners, skip the deploy gate
      }
    }
  }

  if (
    deps.env.provider === "github-actions" &&
    resolvedPr !== undefined &&
    deps.env.token !== undefined &&
    deps.env.repository !== undefined &&
    deps.env.headSha !== undefined &&
    deps.prCheck !== undefined
  ) {
    // Look up any verdict-override Evidence rows for the audit summary.
    // Filter is by task only: ci verify creates a fresh verdict.id on every
    // run, so override rows recorded against an earlier verdict would never
    // match an id-based filter. The latest override for the task is what the
    // PR check should reflect. Conclusion mapping is unchanged — override is
    // auxiliary, not a gate flip.
    let overrides: readonly VerdictOverridePayload[] | undefined;
    try {
      const overrideRows = await deps.evidenceStore.list({
        task_id: taskId,
        kind: "verdict-override",
      });
      overrides = overrideRows.map((r) => r.payload as VerdictOverridePayload);
    } catch {
      // non-fatal — overrides are auxiliary audit; proceed without them
    }

    await postPrCheck(
      {
        verdict,
        repository: deps.env.repository,
        headSha: deps.env.headSha,
        overrides: overrides !== undefined && overrides.length > 0 ? overrides : undefined,
        deployBlockReason,
      },
      deps.prCheck,
    );
  }

  return verdict;
}

function makeDefaultWriteOutput(outputPath: string): (key: string, value: string) => Promise<void> {
  return async (key: string, value: string): Promise<void> => {
    await appendFile(outputPath, `${key}=${value}\n`, "utf8");
  };
}

async function defaultReadTestResults(path: string): Promise<TestResultPayload | undefined> {
  return readJson<TestResultPayload>(path);
}
