import { join } from "node:path";
import { mkdir, writeFile } from "node:fs/promises";
import {
  recordEvidence,
  type EvidenceStorePort,
  type LintViolationPayload,
} from "@/features/evidence";
import type { VerdictStorePort } from "@/features/verdict";
import { checkArchitectureRules } from "@/features/verify";

export interface SessionExitDeps {
  readonly evidenceStore: EvidenceStorePort;
  readonly verdictStore: VerdictStorePort;
  readonly checkDirtyTree?: (repoRoot: string) => Promise<boolean>;
}

export interface SessionExitArgs {
  readonly taskId: string;
  readonly projectRoot: string;
}

export interface SessionExitResult {
  readonly exitCode: 0 | 1 | 2;
  readonly summary: {
    readonly lintViolations: number;
    readonly baselineClean: boolean;
    readonly dirtyTree: boolean;
    readonly verdictDecision?: "PASS" | "FAIL" | "HUMAN" | "BLOCK";
  };
  readonly warnings: readonly string[];
  readonly progressPath: string;
}

export async function sessionExit(
  deps: SessionExitDeps,
  args: SessionExitArgs,
): Promise<SessionExitResult> {
  const { taskId, projectRoot } = args;
  const warnings: string[] = [];

  // 1. Re-run baseline arch lints. Same set as session start.
  const violations = await checkArchitectureRules({ repoRoot: projectRoot });
  const errorViolations = violations.filter((v) => v.severity === "error");
  const lintViolations = errorViolations.length;
  const baselineClean = lintViolations === 0;
  if (!baselineClean) {
    warnings.push(
      `Baseline architecture lint regressed: ${lintViolations} error-severity violation${lintViolations !== 1 ? "s" : ""} present at exit`,
    );
  }
  // Record one lint-violation row per error-severity finding so
  // `task introspect` can list the specific lints alongside the count.
  await Promise.all(
    errorViolations.map((v) => {
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
        witness_level: "witnessed-by-maestro",
        payload,
      });
    }),
  );

  // 2. Latest verdict — warn on FAIL/BLOCK but do not block exit.
  const verdict = await deps.verdictStore.readLatest(taskId);
  if (verdict !== undefined && (verdict.decision === "FAIL" || verdict.decision === "BLOCK")) {
    warnings.push(
      `Latest verdict is ${verdict.decision} (computed at ${verdict.computedAt}) — task is not in a passing state`,
    );
  }

  // 3. Dirty working tree — warn but do not block.
  const dirtyTree = await (deps.checkDirtyTree ?? defaultCheckDirtyTree)(projectRoot);
  if (dirtyTree) {
    warnings.push(
      "Working tree has uncommitted changes — review and commit before declaring the session done",
    );
  }

  // 4. Write progress.md summary.
  const runDir = join(projectRoot, ".maestro", "runs", taskId);
  await mkdir(runDir, { recursive: true });
  const progressPath = join(runDir, "progress.md");
  await writeFile(progressPath, formatProgressMarkdown({
    taskId,
    lintViolations,
    baselineClean,
    dirtyTree,
    verdictDecision: verdict?.decision,
    warnings,
  }));

  // 5. Record session-exit evidence.
  await recordEvidence(deps.evidenceStore, {
    task_id: taskId,
    kind: "session-exit",
    witness_level: "witnessed-by-maestro",
    payload: { taskId, lintViolations, baselineClean, dirtyTree },
  });

  // 6. Resolve exit code: lint regressions strongest signal, then baseline.
  const exitCode: 0 | 1 | 2 = lintViolations > 0 ? 2 : !baselineClean ? 1 : 0;

  return {
    exitCode,
    summary: {
      lintViolations,
      baselineClean,
      dirtyTree,
      ...(verdict?.decision ? { verdictDecision: verdict.decision } : {}),
    },
    warnings,
    progressPath,
  };
}

interface ProgressMarkdownInput {
  readonly taskId: string;
  readonly lintViolations: number;
  readonly baselineClean: boolean;
  readonly dirtyTree: boolean;
  readonly verdictDecision?: "PASS" | "FAIL" | "HUMAN" | "BLOCK";
  readonly warnings: readonly string[];
}

function formatProgressMarkdown(input: ProgressMarkdownInput): string {
  const lines: string[] = [];
  lines.push(`# Session exit progress — task ${input.taskId}`);
  lines.push("");
  lines.push(`- Lint violations (error): ${input.lintViolations}`);
  lines.push(`- Baseline arch-lint clean: ${input.baselineClean}`);
  lines.push(`- Working tree dirty: ${input.dirtyTree}`);
  if (input.verdictDecision !== undefined) {
    lines.push(`- Latest verdict: ${input.verdictDecision}`);
  }
  if (input.warnings.length > 0) {
    lines.push("");
    lines.push("## Warnings");
    for (const w of input.warnings) lines.push(`- ${w}`);
  }
  return lines.join("\n") + "\n";
}

async function defaultCheckDirtyTree(repoRoot: string): Promise<boolean> {
  const proc = Bun.spawnSync({
    cmd: ["git", "status", "--porcelain"],
    cwd: repoRoot,
    stdout: "pipe",
    stderr: "pipe",
  });
  if (proc.exitCode !== 0) return false;
  const output = new TextDecoder().decode(proc.stdout).trim();
  return output.length > 0;
}
