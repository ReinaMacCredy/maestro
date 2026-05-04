import { appendFile } from "node:fs/promises";
import { readJson } from "@/shared/lib/fs.js";
import type { EvidenceStorePort } from "@/features/evidence/ports/storage.js";
import { recordEvidence } from "@/features/evidence/index.js";
import type { CommandPayload, VerdictOverridePayload } from "@/features/evidence/domain/types.js";
import { requestVerdict } from "@/features/verdict/index.js";
import type { RequestVerdictDeps } from "@/features/verdict/index.js";
import type { Verdict } from "@/features/verdict/domain/types.js";
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
  const verdict = await deps.verdict.request(
    { taskId, base: resolvedBase, pr: resolvedPr },
    deps.verdictDeps,
  );

  const outputPath = deps.env.outputPath;
  if (typeof outputPath === "string" && outputPath.length > 0) {
    const writeOutput = deps.writeOutput ?? makeDefaultWriteOutput(outputPath);
    await writeOutput("verdict_id", verdict.id);
    await writeOutput("verdict_decision", verdict.decision);
    await writeOutput("effective_risk_class", verdict.effectiveRiskClass);
  }

  if (
    deps.env.provider === "github-actions" &&
    deps.env.pr !== undefined &&
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
