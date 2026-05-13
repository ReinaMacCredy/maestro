import { join } from "node:path";
import { rm } from "node:fs/promises";
import { MaestroError } from "@/shared/errors.js";
import { execArgv } from "@/shared/lib/shell.js";
import { recordEvidence, type EvidenceStorePort } from "@/features/evidence";
import type { VerdictStorePort } from "@/features/verdict";

export interface RecoverDeps {
  readonly evidenceStore: EvidenceStorePort;
  readonly verdictStore: VerdictStorePort;
  readonly resolveHeadCommit?: (cwd: string) => Promise<string>;
  readonly resolveCommitForTree?: (cwd: string, treeSha: string) => Promise<string | undefined>;
  readonly resolveRef?: (cwd: string, ref: string) => Promise<string | undefined>;
  readonly checkDirtyTree?: (cwd: string) => Promise<boolean>;
  readonly resetHard?: (cwd: string, commit: string) => Promise<void>;
}

export interface RecoverArgs {
  readonly taskId: string;
  readonly projectRoot: string;
  readonly to?: string;
  readonly force?: boolean;
  readonly dryRun?: boolean;
}

export interface RecoverPlan {
  readonly fromCommit: string;
  readonly toCommit: string;
  readonly anchorVerdictId?: string;
  readonly anchorTreeSha?: string;
  readonly reason: "verdict-anchored" | "explicit-ref" | "head-revert";
  readonly runStatePath: string;
  readonly dirty: boolean;
}

export interface RecoverResult {
  readonly plan: RecoverPlan;
  readonly applied: boolean;
  readonly evidenceId?: string;
}

export async function recoverTask(
  deps: RecoverDeps,
  args: RecoverArgs,
): Promise<RecoverResult> {
  const fromCommit = await (deps.resolveHeadCommit ?? defaultResolveHeadCommit)(args.projectRoot);
  if (!fromCommit) {
    throw new MaestroError(
      "Could not resolve current HEAD commit",
      ["Ensure the project is a git repository (`git status`)."],
      "recover-no-head",
    );
  }

  const plan = await buildPlan(deps, args, fromCommit);

  const dirty = await (deps.checkDirtyTree ?? defaultCheckDirtyTree)(args.projectRoot);
  const dirtyPlan: RecoverPlan = { ...plan, dirty };

  if (args.dryRun === true) {
    return { plan: dirtyPlan, applied: false };
  }

  if (dirty && args.force !== true) {
    throw new MaestroError(
      "Working tree has uncommitted changes; refusing to reset",
      [
        "Commit or stash changes first, then re-run.",
        "Pass `--force` to override (destructive).",
      ],
      "recover-dirty-tree",
    );
  }

  if (plan.fromCommit === plan.toCommit) {
    return { plan: dirtyPlan, applied: false };
  }

  await (deps.resetHard ?? defaultResetHard)(args.projectRoot, plan.toCommit);

  let droppedRunState = false;
  try {
    await rm(plan.runStatePath, { recursive: true, force: true });
    droppedRunState = true;
  } catch {
    droppedRunState = false;
  }

  const evidence = await recordEvidence(deps.evidenceStore, {
    task_id: args.taskId,
    kind: "recovery",
    witness_level: "witnessed-by-maestro",
    payload: {
      taskId: args.taskId,
      fromCommit: plan.fromCommit,
      toCommit: plan.toCommit,
      anchorVerdictId: plan.anchorVerdictId,
      droppedRunState,
      reason: plan.reason,
    },
  });

  return { plan: dirtyPlan, applied: true, evidenceId: evidence.id };
}

async function buildPlan(
  deps: RecoverDeps,
  args: RecoverArgs,
  fromCommit: string,
): Promise<RecoverPlan> {
  const runStatePath = join(args.projectRoot, ".maestro", "runs", args.taskId);

  if (typeof args.to === "string" && args.to.length > 0) {
    const resolved = await (deps.resolveRef ?? defaultResolveRef)(args.projectRoot, args.to);
    if (resolved === undefined) {
      throw new MaestroError(
        `Could not resolve git ref \`${args.to}\``,
        [`Run \`git rev-parse ${args.to}\` to inspect.`],
        "recover-bad-ref",
      );
    }
    return {
      fromCommit,
      toCommit: resolved,
      reason: "explicit-ref",
      runStatePath,
      dirty: false,
    };
  }

  const history = await deps.verdictStore.history(args.taskId);
  const lastPass = [...history].reverse().find((v) => v.decision === "PASS");
  if (lastPass === undefined) {
    throw new MaestroError(
      `No PASS verdict on record for task ${args.taskId}`,
      [
        "Pass `--to <commit>` explicitly, or",
        `request a verdict first: \`maestro verdict request --task ${args.taskId}\``,
      ],
      "recover-no-pass-verdict",
    );
  }
  const treeSha = lastPass.subject?.tree_sha;
  if (treeSha === undefined) {
    throw new MaestroError(
      `Latest PASS verdict for ${args.taskId} has no tree_sha`,
      ["Pass `--to <commit>` explicitly."],
      "recover-no-tree-sha",
    );
  }
  const commit = await (deps.resolveCommitForTree ?? defaultResolveCommitForTree)(
    args.projectRoot,
    treeSha,
  );
  if (commit === undefined) {
    throw new MaestroError(
      `Could not find a commit with tree ${treeSha.slice(0, 12)} on this branch`,
      [
        "The verdict's tree has been garbage-collected or rewritten.",
        "Pass `--to <commit>` to choose a recovery target manually.",
      ],
      "recover-tree-not-found",
    );
  }
  return {
    fromCommit,
    toCommit: commit,
    anchorVerdictId: lastPass.id,
    anchorTreeSha: treeSha,
    reason: "verdict-anchored",
    runStatePath,
    dirty: false,
  };
}

async function defaultResolveHeadCommit(cwd: string): Promise<string> {
  const r = await execArgv(["git", "rev-parse", "HEAD"], { cwd });
  return r.exitCode === 0 ? r.stdout : "";
}

async function defaultResolveRef(cwd: string, ref: string): Promise<string | undefined> {
  const r = await execArgv(["git", "rev-parse", "--verify", `${ref}^{commit}`], { cwd });
  return r.exitCode === 0 && r.stdout.length > 0 ? r.stdout : undefined;
}

async function defaultResolveCommitForTree(
  cwd: string,
  treeSha: string,
): Promise<string | undefined> {
  const r = await execArgv(["git", "log", "--all", "--format=%H %T"], { cwd });
  if (r.exitCode !== 0) return undefined;
  for (const line of r.stdout.split("\n")) {
    const parts = line.split(" ");
    if (parts.length === 2 && parts[1] === treeSha) return parts[0];
  }
  return undefined;
}

async function defaultCheckDirtyTree(cwd: string): Promise<boolean> {
  const r = await execArgv(["git", "status", "--porcelain"], { cwd });
  if (r.exitCode !== 0) return false;
  return r.stdout.length > 0;
}

async function defaultResetHard(cwd: string, commit: string): Promise<void> {
  const r = await execArgv(["git", "reset", "--hard", commit], { cwd });
  if (r.exitCode !== 0) {
    throw new MaestroError(
      `git reset --hard ${commit} failed`,
      [r.stderr],
      "recover-reset-failed",
    );
  }
}
