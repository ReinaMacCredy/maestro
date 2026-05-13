import { join } from "node:path";
import { mkdir, writeFile, readFile } from "node:fs/promises";
import { MaestroError } from "@/shared/errors.js";
import { recordEvidence, type EvidenceStorePort } from "@/features/evidence";
import {
  composeTaskIntrospection,
  formatTaskIntrospectionMarkdown,
  type TaskIntrospectionDeps,
  type TaskIntrospectionView,
} from "@/features/task";
import {
  checkArchitectureRules,
  type ArchitectureViolation,
} from "@/features/verify";

export interface SessionStartDeps extends TaskIntrospectionDeps {
  readonly evidenceStore: EvidenceStorePort;
  readonly resolveHeadSha?: (repoRoot: string) => Promise<string>;
  readonly runScript?: (
    repoRoot: string,
    scriptName: string,
  ) => Promise<{ exitCode: number; stdout: string; stderr: string } | undefined>;
}

export interface SessionStartArgs {
  readonly taskId: string;
  readonly projectRoot: string;
}

export interface SessionStartResult {
  readonly view: TaskIntrospectionView;
  readonly orientPath: string;
  readonly headSha: string;
}

const ORIENT_HEADER = `# Maestro session orient digest\n\n`;

export async function sessionStart(
  deps: SessionStartDeps,
  args: SessionStartArgs,
): Promise<SessionStartResult> {
  const { taskId, projectRoot } = args;

  // 1. Compose the introspection view first so that "task not found" surfaces
  //    before any heavier work (verifier, scripts).
  const view = await composeTaskIntrospection(deps, taskId);

  // 2. Baseline arch-lint check. We do NOT call runTrustVerifier here because
  //    that path requires a Contract; the architecture lints are the strict
  //    repo-shape invariants and are sufficient for a baseline gate at session
  //    start. Other Trust Verifier checks fire during `task verify` once a
  //    contract exists.
  const violations = await checkArchitectureRules({ repoRoot: projectRoot });
  const errorViolations = violations.filter((v) => v.severity === "error");
  if (errorViolations.length > 0) {
    throw blockOnArchViolations(errorViolations);
  }

  // 3. Optional setup/verify scripts (only if defined in package.json).
  await runOptionalScript(deps, projectRoot, "maestro:setup");
  await runOptionalScript(deps, projectRoot, "maestro:verify");

  // 4. Resolve HEAD sha — anchor for "recent commits" in C-1.
  const headSha = await (deps.resolveHeadSha ?? defaultResolveHeadSha)(projectRoot);

  // 5. Write orient.md under .maestro/runs/<taskId>/.
  const runDir = join(projectRoot, ".maestro", "runs", taskId);
  await mkdir(runDir, { recursive: true });
  const orientPath = join(runDir, "orient.md");
  const body = ORIENT_HEADER + formatTaskIntrospectionMarkdown(view) + "\n";
  await writeFile(orientPath, body);

  // 6. Record session-start evidence.
  await recordEvidence(deps.evidenceStore, {
    task_id: taskId,
    kind: "session-start",
    witness_level: "witnessed-by-maestro",
    payload: { taskId, headSha },
  });

  return { view, orientPath, headSha };
}

function blockOnArchViolations(
  violations: readonly ArchitectureViolation[],
): MaestroError {
  const summary = `Baseline architecture lint blocks session start: ${violations.length} error-severity violation${violations.length !== 1 ? "s" : ""}`;
  const hints: string[] = [];
  for (const v of violations) {
    const loc = v.line !== undefined ? `${v.file}:${v.line}` : v.file;
    hints.push(`${v.ruleId} at ${loc} — ${v.message}`);
  }
  hints.push(
    "Recover by `git reset --hard <last-green-tag>` or by reverting the unhealthy commit. The explicit `maestro recover` verb arrives in Phase 2.",
  );
  return new MaestroError(summary, hints, "session-start-baseline-blocked");
}

async function runOptionalScript(
  deps: SessionStartDeps,
  projectRoot: string,
  scriptName: string,
): Promise<void> {
  const has = await packageJsonHasScript(projectRoot, scriptName);
  if (!has) return;
  const result = await (deps.runScript ?? defaultRunScript)(projectRoot, scriptName);
  if (result === undefined) return;
  if (result.exitCode !== 0) {
    throw new MaestroError(
      `\`${scriptName}\` failed during session start (exit ${result.exitCode})`,
      [
        `Re-run \`bun run ${scriptName}\` to see full output.`,
        result.stderr.trim() ? `stderr: ${result.stderr.trim().split("\n")[0] ?? ""}` : "",
      ].filter(Boolean) as string[],
      "session-start-script-failed",
    );
  }
}

async function packageJsonHasScript(
  projectRoot: string,
  scriptName: string,
): Promise<boolean> {
  try {
    const text = await readFile(join(projectRoot, "package.json"), "utf8");
    const pkg = JSON.parse(text) as { scripts?: Record<string, string> };
    return typeof pkg.scripts?.[scriptName] === "string";
  } catch {
    return false;
  }
}

async function defaultResolveHeadSha(repoRoot: string): Promise<string> {
  const proc = Bun.spawnSync({
    cmd: ["git", "rev-parse", "HEAD"],
    cwd: repoRoot,
    stdout: "pipe",
    stderr: "pipe",
  });
  if (proc.exitCode !== 0) return "";
  return new TextDecoder().decode(proc.stdout).trim();
}

async function defaultRunScript(
  repoRoot: string,
  scriptName: string,
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  const proc = Bun.spawnSync({
    cmd: ["bun", "run", scriptName],
    cwd: repoRoot,
    stdout: "pipe",
    stderr: "pipe",
  });
  return {
    exitCode: proc.exitCode ?? 0,
    stdout: new TextDecoder().decode(proc.stdout),
    stderr: new TextDecoder().decode(proc.stderr),
  };
}
