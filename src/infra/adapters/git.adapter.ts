import type { GitFileChange, GitState } from "@/infra/domain/git-types.js";
import type { GitPort } from "../ports/git.port.js";
import { exec } from "@/shared/lib/shell.js";

export class ShellGitAdapter implements GitPort {
  async getState(cwd: string): Promise<GitState> {
    const [branchResult, logResult, statusResult, diffResult] =
      await Promise.all([
        exec("git branch --show-current", { cwd }),
        exec("git log --oneline -10", { cwd }),
        exec("git status --porcelain", { cwd }),
        exec("git diff --stat HEAD", { cwd }),
      ]);

    const branch = branchResult.stdout || "HEAD";
    const recentCommits = logResult.stdout
      ? logResult.stdout.split("\n")
      : [];
    const changedFiles = statusResult.stdout
      ? statusResult.stdout
          .split("\n")
          .map((line) => line.slice(3).trim())
          .filter(Boolean)
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
    const result = await exec("git rev-parse --is-inside-work-tree", { cwd });
    return result.exitCode === 0 && result.stdout === "true";
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
