import { dirname, basename, join } from "node:path";
import type { GitFileChange, GitState, GitWorktree } from "@/infra/domain/git-types.js";
import type { GitPort } from "../ports/git.port.js";
import { execArgv } from "@/shared/lib/shell.js";
import { dirExists } from "@/shared/lib/fs.js";
import { execOrThrow } from "@/shared/lib/shell.js";

export class ShellGitAdapter implements GitPort {
  async getState(cwd: string): Promise<GitState> {
    const [branchResult, logResult, statusResult, diffResult] =
      await Promise.all([
        execArgv(["git", "branch", "--show-current"], { cwd }),
        execArgv(["git", "log", "--oneline", "-10"], { cwd }),
        execArgv(["git", "status", "--porcelain"], { cwd }),
        execArgv(["git", "diff", "--stat", "HEAD"], { cwd }),
      ]);

    const branch = branchResult.stdout || "HEAD";
    const recentCommits = logResult.stdout
      ? logResult.stdout.split("\n")
      : [];
    const changedFiles = statusResult.stdout
      ? statusResult.stdout
          .split("\n")
          .map(parsePorcelainPath)
          .filter((value): value is string => value !== undefined)
      : [];
    const workingTreeClean = statusResult.stdout === "";

    const diffStat = parseDiffStat(diffResult.stdout);

    return {
      branch,
      recentCommits,
      changedFiles,
      fileChanges: statusResult.stdout ? parseGitFileChanges(statusResult.stdout) : [],
      workingTreeClean,
      diffStat,
    };
  }

  async isRepo(cwd: string): Promise<boolean> {
    const result = await execArgv(["git", "rev-parse", "--is-inside-work-tree"], { cwd });
    return result.exitCode === 0 && result.stdout === "true";
  }

  async getCurrentBranch(cwd: string): Promise<string> {
    const result = await execArgv(["git", "branch", "--show-current"], { cwd });
    return result.stdout || "HEAD";
  }

  async createWorktree(
    cwd: string,
    input: {
      readonly slug: string;
      readonly baseBranch: string;
      readonly branchPrefix: string;
    },
  ): Promise<GitWorktree> {
    const repoRoot = await this.getRepoRoot(cwd);
    const repoName = basename(repoRoot);
    const parentDir = dirname(repoRoot);

    for (let index = 0; index < 100; index += 1) {
      const suffix = index === 0 ? "" : `-${index + 1}`;
      const effectiveSlug = `${input.slug}${suffix}`;
      const branch = `${input.branchPrefix}/${effectiveSlug}`;
      const path = join(parentDir, `${repoName}-${effectiveSlug}`);
      const [branchTaken, dirTaken] = await Promise.all([
        this.branchExists(repoRoot, branch),
        dirExists(path),
      ]);
      if (branchTaken || dirTaken) {
        continue;
      }

      await execOrThrow(
        ["git", "worktree", "add", "-b", branch, path, input.baseBranch],
        "git worktree add",
        { cwd: repoRoot },
      );

      return {
        slug: effectiveSlug,
        baseBranch: input.baseBranch,
        branch,
        path,
      };
    }

    throw new Error(`Unable to create a unique worktree for slug '${input.slug}'`);
  }

  private async branchExists(cwd: string, branch: string): Promise<boolean> {
    const result = await execArgv(
      ["git", "show-ref", "--verify", "--quiet", `refs/heads/${branch}`],
      { cwd },
    );
    return result.exitCode === 0;
  }

  private async getRepoRoot(cwd: string): Promise<string> {
    const result = await execOrThrow(
      ["git", "rev-parse", "--show-toplevel"],
      "git rev-parse --show-toplevel",
      { cwd },
    );
    return result.stdout;
  }
}

function parseGitFileChanges(output: string): GitFileChange[] {
  return output
    .split("\n")
    .map((line) => line.trimEnd())
    .filter(Boolean)
    .map((line) => {
      const status = line.slice(0, 2);
      const rawPath = line.slice(3).trim();
      const path = status.includes("R") || status.includes("C")
        ? rawPath.split(" -> ").at(-1) ?? rawPath
        : rawPath;
      return {
        path,
        kind: classifyGitFileChange(status),
      } satisfies GitFileChange;
    });
}

/**
 * Parse a single `git status --porcelain` line into its path.
 *
 * Porcelain format: `XY <path>` where X is index status, Y is worktree
 * status, and there is always one space separator before the path.
 * Renames use `XY <orig> -> <new>`.
 *
 * We cannot simply `slice(3)` because `execSpawn` trims the overall stdout,
 * which strips the leading space from the first line when X is absent
 * (`" M path"` becomes `"M path"`). Instead, match the XY-status pattern
 * and return the filename portion. If the line does not match the porcelain
 * shape (empty or malformed), return undefined so it is filtered out.
 */
function parsePorcelainPath(line: string): string | undefined {
  if (line.length === 0) return undefined;
  // Pad a missing leading space so the slice offset is stable regardless of
  // whether the first character is whitespace (trimmed by the shell).
  const padded = line.length >= 3 && line[2] === " " ? line : " " + line;
  const body = padded.slice(3).trim();
  if (body.length === 0) return undefined;
  // Rename/copy: "ORIG -> NEW". Prefer the new path. Skip the regex on the
  // common path to avoid per-line allocation.
  if (body.includes(" -> ")) {
    const renameMatch = body.match(/^(.*?) -> (.+)$/);
    if (renameMatch) return renameMatch[2];
  }
  return body;
}

function classifyGitFileChange(status: string): GitFileChange["kind"] {
  if (status === "??") return "untracked";
  if (status.includes("U")) return "conflicted";
  if (status.includes("R")) return "renamed";
  if (status.includes("C")) return "copied";
  if (status.includes("T")) return "typechange";
  if (status.includes("A")) return "added";
  if (status.includes("D")) return "deleted";
  return "modified";
}

function parseDiffStat(output: string): string {
  if (!output) return "+0 -0";
  const lines = output.split("\n");
  const summary = lines[lines.length - 1];
  if (!summary) return "+0 -0";

  const insertions = summary.match(/(\d+) insertion/);
  const deletions = summary.match(/(\d+) deletion/);
  const ins = insertions?.[1] ?? "0";
  const del = deletions?.[1] ?? "0";
  return `+${ins} -${del}`;
}
