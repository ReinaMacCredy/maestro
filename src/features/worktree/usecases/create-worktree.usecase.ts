import { join } from "node:path";
import type { GitPort } from "@/infra/ports/git.port.js";
import type { GitWorktree } from "@/infra/domain/git-types.js";
import { ensureDir } from "@/shared/lib/fs.js";

export interface CreateWorktreeDeps {
  readonly git: GitPort;
}

export interface CreateWorktreeArgs {
  readonly cwd: string;
  readonly slug: string;
  readonly baseBranch?: string;
  readonly branchPrefix?: string;
}

export interface CreateWorktreeResult {
  readonly worktree: GitWorktree;
  readonly runsDir: string;
}

export async function createWorktreeForTask(
  deps: CreateWorktreeDeps,
  args: CreateWorktreeArgs,
): Promise<CreateWorktreeResult> {
  const baseBranch = args.baseBranch ?? "main";
  const branchPrefix = args.branchPrefix ?? "feat";
  const worktree = await deps.git.createWorktree(args.cwd, {
    slug: args.slug,
    baseBranch,
    branchPrefix,
  });
  const runsDir = join(worktree.path, ".maestro", "runs");
  await ensureDir(runsDir);
  return { worktree, runsDir };
}

export function formatCreateWorktreeLines(r: CreateWorktreeResult): string[] {
  return [
    `Created worktree: ${r.worktree.slug}`,
    `  Path:   ${r.worktree.path}`,
    `  Branch: ${r.worktree.branch}`,
    `  Base:   ${r.worktree.baseBranch}`,
    `  Runs:   ${r.runsDir}`,
    "",
    `Next: cd ${r.worktree.path} && maestro session start <task-id>`,
  ];
}
