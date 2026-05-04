import { appendFile } from "node:fs/promises";
import type { EvidenceStorePort } from "@/features/evidence/ports/storage.js";
import { recordEvidence } from "@/features/evidence/index.js";
import type { CommandPayload } from "@/features/evidence/domain/types.js";
import { requestVerdict } from "@/features/verdict/index.js";
import type { RequestVerdictDeps } from "@/features/verdict/index.js";
import type { Verdict } from "@/features/verdict/domain/types.js";
import type { CiEnv } from "../domain/ci-env.js";

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

  // Ingest test results if available
  const testResultsPath = args.testResultsPath ?? process.env.CI_TEST_RESULTS_FILE;
  if (typeof testResultsPath === "string" && testResultsPath.length > 0) {
    const readTestResults = deps.readTestResults ?? defaultReadTestResults;
    try {
      const results = await readTestResults(testResultsPath);
      if (results !== undefined) {
        const commandSummary = buildTestResultsSummary(results);
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
        void commandSummary; // consumed via structured payload above; summary for future use
      }
    } catch {
      // test results ingestion failure is non-fatal; continue to verdict
    }
  }

  const verdict = await deps.verdict.request(
    { taskId, base: resolvedBase },
    deps.verdictDeps,
  );

  // Write GITHUB_OUTPUT keys if outputPath is set
  const outputPath = deps.env.outputPath;
  if (typeof outputPath === "string" && outputPath.length > 0) {
    const writeOutput = deps.writeOutput ?? makeDefaultWriteOutput(outputPath);
    await writeOutput("verdict_id", verdict.id);
    await writeOutput("verdict_decision", verdict.decision);
    await writeOutput("effective_risk_class", verdict.effectiveRiskClass);
  }

  return verdict;
}

function buildTestResultsSummary(results: TestResultPayload): string {
  const total = results.total ?? results.passed + results.failed + (results.skipped ?? 0);
  return `${results.passed}/${total} passed, ${results.failed} failed${results.skipped !== undefined ? `, ${results.skipped} skipped` : ""}`;
}

function makeDefaultWriteOutput(outputPath: string): (key: string, value: string) => Promise<void> {
  return async (key: string, value: string): Promise<void> => {
    await appendFile(outputPath, `${key}=${value}\n`, "utf8");
  };
}

async function defaultReadTestResults(path: string): Promise<TestResultPayload | undefined> {
  try {
    const { readFile } = await import("node:fs/promises");
    const text = await readFile(path, "utf8");
    return JSON.parse(text) as TestResultPayload;
  } catch {
    return undefined;
  }
}
