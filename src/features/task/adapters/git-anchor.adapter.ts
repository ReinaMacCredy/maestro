import { execArgv } from "@/shared/lib/shell.js";
import { normalizeSlashes } from "@/shared/lib/path-normalize.js";
import type { GitAnchorPort, GitTouchedFilesResult } from "../ports/git-anchor.port.js";

const EMPTY_TREE_HASH = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
const MAX_STORED_TOUCHED_FILES = 2_000;

export class ShellGitAnchorAdapter implements GitAnchorPort {
  async resolveRepoRoot(cwd: string): Promise<string> {
    const result = await execArgv(["git", "rev-parse", "--show-toplevel"], { cwd });
    return result.exitCode === 0 && result.stdout ? result.stdout : cwd;
  }

  async resolveHeadCommit(cwd: string): Promise<string | undefined> {
    const repo = await execArgv(["git", "rev-parse", "--is-inside-work-tree"], { cwd });
    if (repo.exitCode !== 0 || repo.stdout !== "true") {
      return undefined;
    }

    const head = await execArgv(["git", "rev-parse", "HEAD"], { cwd });
    if (head.exitCode === 0 && head.stdout) {
      return head.stdout;
    }
    return EMPTY_TREE_HASH;
  }

  async collectTouchedFiles(input: {
    readonly repoRoot: string;
    readonly claimedAtCommit?: string;
    readonly rebaseFallback: "best-effort" | "fail";
  }): Promise<GitTouchedFilesResult> {
    const head = await this.resolveHeadCommit(input.repoRoot);
    if (!head) {
      return {
        gitAvailable: false,
        actualFilesTouched: [],
        notes: "Git is unavailable for this contract window.",
      };
    }

    const anchorResolution = await this.resolveAnchor(input.repoRoot, input.claimedAtCommit, head, input.rebaseFallback);
    if (anchorResolution.anchorFallback === "lost") {
      return {
        gitAvailable: true,
        actualFilesTouched: [],
        closedAtCommit: head,
        anchorFallback: "lost",
        notes: anchorResolution.notes,
      };
    }

    const [range, workingTree, staged, untracked, mergeSourcedFiles] = await Promise.all([
      anchorResolution.anchor
        ? execArgv(["git", "diff", "--name-only", anchorResolution.anchor, head], { cwd: input.repoRoot })
        : Promise.resolve({ stdout: "", stderr: "", exitCode: 0 }),
      execArgv(["git", "diff", "--name-only"], { cwd: input.repoRoot }),
      execArgv(["git", "diff", "--cached", "--name-only"], { cwd: input.repoRoot }),
      execArgv(["git", "ls-files", "--others", "--exclude-standard"], { cwd: input.repoRoot }),
      anchorResolution.anchor
        ? this.collectMergeSourcedFiles(input.repoRoot, anchorResolution.anchor, head)
        : Promise.resolve([] as readonly string[]),
    ]);

    const files = new Set<string>();
    for (const output of [range.stdout, workingTree.stdout, staged.stdout, untracked.stdout]) {
      for (const path of splitPaths(output)) {
        if (!isContractRuntimePath(path)) {
          files.add(path);
        }
      }
    }

    const notes = [
      anchorResolution.notes,
      workingTree.stdout ? "Includes uncommitted tracked changes." : undefined,
      staged.stdout ? "Includes staged changes." : undefined,
      untracked.stdout ? "Includes untracked working-tree files." : undefined,
      mergeSourcedFiles.length > 0 ? formatMergeSourcedFilesNote(mergeSourcedFiles) : undefined,
    ].filter((value): value is string => Boolean(value));

    return {
      gitAvailable: true,
      actualFilesTouched: [...files].sort(),
      ...(files.size > MAX_STORED_TOUCHED_FILES
        ? {
            actualFilesTouchedTruncated: {
              stored: MAX_STORED_TOUCHED_FILES,
              actual: files.size,
            },
          }
        : {}),
      closedAtCommit: head,
      anchorFallback: anchorResolution.anchorFallback,
      ...(notes.length > 0 ? { notes: notes.join(" ") } : {}),
    };
  }

  async windowsOverlap(input: {
    readonly repoRoot: string;
    readonly left: {
      readonly claimedAtCommit?: string;
      readonly closedAtCommit?: string;
    };
    readonly right: {
      readonly claimedAtCommit?: string;
      readonly closedAtCommit?: string;
    };
  }): Promise<boolean | undefined> {
    const head = await this.resolveHeadCommit(input.repoRoot);
    if (!head) {
      return undefined;
    }

    const leftWindow = this.resolveWindow(input.left, head);
    const rightWindow = this.resolveWindow(input.right, head);
    if (!leftWindow || !rightWindow) {
      return undefined;
    }
    const commitsPresent = await Promise.all([
      this.commitExists(input.repoRoot, leftWindow.start),
      this.commitExists(input.repoRoot, leftWindow.end),
      this.commitExists(input.repoRoot, rightWindow.start),
      this.commitExists(input.repoRoot, rightWindow.end),
    ]);
    if (commitsPresent.some((present) => !present)) {
      return undefined;
    }

    const [leftBeforeRight, rightBeforeLeft] = await Promise.all([
      this.isStrictAncestor(input.repoRoot, leftWindow.end, rightWindow.start),
      this.isStrictAncestor(input.repoRoot, rightWindow.end, leftWindow.start),
    ]);

    return !leftBeforeRight && !rightBeforeLeft;
  }

  private async resolveAnchor(
    cwd: string,
    claimedAtCommit: string | undefined,
    head: string,
    rebaseFallback: "best-effort" | "fail",
  ): Promise<{
    readonly anchor?: string;
    readonly anchorFallback?: "direct" | "reflog" | "merge-base" | "lost";
    readonly notes?: string;
  }> {
    if (!claimedAtCommit) {
      return {
        anchor: head,
        anchorFallback: "direct",
      };
    }
    if (claimedAtCommit === EMPTY_TREE_HASH) {
      return {
        anchor: claimedAtCommit,
        anchorFallback: "direct",
      };
    }
    if (claimedAtCommit === head) {
      return {
        anchor: claimedAtCommit,
        anchorFallback: "direct",
      };
    }

    const reachable = await execArgv(["git", "merge-base", "--is-ancestor", claimedAtCommit, head], { cwd });
    if (reachable.exitCode === 0) {
      return {
        anchor: claimedAtCommit,
        anchorFallback: "direct",
      };
    }
    if (rebaseFallback === "fail") {
      return {
        anchorFallback: "lost",
        notes: "Claim anchor is no longer reachable from HEAD.",
      };
    }

    const reflogCommit = await this.findReflogCommit(cwd, claimedAtCommit);
    if (reflogCommit) {
      return {
        anchor: reflogCommit,
        anchorFallback: "reflog",
        notes: "Recovered claim anchor from git reflog even though it is no longer an ancestor of HEAD.",
      };
    }

    const mergeBase = await execArgv(["git", "merge-base", claimedAtCommit, head], { cwd });
    if (mergeBase.exitCode === 0 && mergeBase.stdout) {
      return {
        anchor: mergeBase.stdout,
        anchorFallback: "merge-base",
        notes: `Claim anchor was lost; fell back to merge-base ${mergeBase.stdout}.`,
      };
    }

    return {
      anchorFallback: "lost",
      notes: "Claim anchor could not be recovered after history rewriting.",
    };
  }

  private async findReflogCommit(cwd: string, claimedAtCommit: string): Promise<string | undefined> {
    const reflog = await execArgv(["git", "reflog", "--all", "--format=%H"], { cwd });
    if (reflog.exitCode !== 0 || !reflog.stdout) {
      return undefined;
    }

    return reflog.stdout
      .split("\n")
      .map((line) => line.trim())
      .find((entry) => entry === claimedAtCommit || entry.startsWith(claimedAtCommit));
  }

  private resolveWindow(
    window: {
      readonly claimedAtCommit?: string;
      readonly closedAtCommit?: string;
    },
    head: string,
  ): { readonly start: string; readonly end: string } | undefined {
    const start = window.claimedAtCommit;
    const end = window.closedAtCommit ?? head;
    if (!start || !end) {
      return undefined;
    }
    return { start, end };
  }

  private async isStrictAncestor(cwd: string, older: string, newer: string): Promise<boolean> {
    if (older === newer) {
      return false;
    }
    const result = await execArgv(["git", "merge-base", "--is-ancestor", older, newer], { cwd });
    return result.exitCode === 0;
  }

  private async commitExists(cwd: string, commit: string): Promise<boolean> {
    const result = await execArgv(["git", "cat-file", "-e", `${commit}^{commit}`], { cwd });
    return result.exitCode === 0;
  }

  private async collectMergeSourcedFiles(cwd: string, anchor: string, head: string): Promise<readonly string[]> {
    const merges = await execArgv(["git", "rev-list", "--merges", `${anchor}..${head}`], { cwd });
    if (merges.exitCode !== 0 || !merges.stdout) {
      return [];
    }

    const mergeCommits = merges.stdout.split("\n").map((line) => line.trim()).filter(Boolean);
    const diffs = await Promise.all(
      mergeCommits.map((mergeCommit) =>
        execArgv(["git", "diff-tree", "--no-commit-id", "--name-only", "-r", "-m", mergeCommit], { cwd }),
      ),
    );

    const files = new Set<string>();
    for (const diff of diffs) {
      if (diff.exitCode !== 0) continue;
      for (const path of splitPaths(diff.stdout)) {
        files.add(path);
      }
    }

    return [...files].sort();
  }
}

function isContractRuntimePath(path: string): boolean {
  const normalized = normalizeSlashes(path);
  return normalized === ".maestro/tasks/tasks.jsonl"
    || normalized === ".maestro/tasks/NOW.md"
    || normalized === ".maestro/tasks/.tasks.lock"
    || normalized.startsWith(".maestro/tasks/continuations/")
    || normalized.startsWith(".maestro/tasks/local-history/")
    || normalized.startsWith(".maestro/tasks/contracts/")
    || normalized.startsWith(".maestro/tasks/batches/")
    || normalized.startsWith(".maestro/tasks/candidates/");
}

function splitPaths(output: string): readonly string[] {
  return output
    .split("\n")
    .map((line) => normalizeSlashes(line.trim()))
    .filter((line) => line.length > 0);
}

function formatMergeSourcedFilesNote(files: readonly string[]): string {
  const displayed = files.slice(0, 20);
  const suffix = files.length > displayed.length ? `, +${files.length - displayed.length} more` : "";
  return `Merge-sourced files: ${displayed.join(", ")}${suffix}.`;
}
