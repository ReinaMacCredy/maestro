import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { ShellGitAnchorAdapter } from "@/features/task/adapters/git-anchor.adapter.js";
import { initGitRepo, runCommand } from "../../../../helpers/command-runner.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-git-anchor-"));
  await initGitRepo(tmpDir);
  await runCommand(["git", "config", "user.email", "test@example.com"], tmpDir);
  await runCommand(["git", "config", "user.name", "Test User"], tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function commitFile(path: string, content: string, message: string): Promise<void> {
  await Bun.write(join(tmpDir, path), content);
  await runCommand(["git", "add", path], tmpDir);
  const committed = await runCommand(["git", "commit", "-m", message], tmpDir);
  if (committed.exitCode !== 0) {
    throw new Error(committed.stderr || committed.stdout);
  }
}

describe("ShellGitAnchorAdapter", () => {
  it("returns a criteria-only result outside git repositories", async () => {
    const nonGitDir = await mkdtemp(join(tmpdir(), "maestro-git-anchor-non-git-"));
    try {
      const result = await new ShellGitAnchorAdapter().collectTouchedFiles({
        repoRoot: nonGitDir,
        rebaseFallback: "best-effort",
      });

      expect(result.gitAvailable).toBe(false);
      expect(result.actualFilesTouched).toEqual([]);
      expect(result.notes).toContain("Git is unavailable");
    } finally {
      await rm(nonGitDir, { recursive: true, force: true });
    }
  });

  it("recovers an unreachable claim anchor from the reflog", async () => {
    await commitFile("base.txt", "base\n", "base");
    const anchor = (await runCommand(["git", "rev-parse", "HEAD"], tmpDir)).stdout;
    await commitFile("later.txt", "later\n", "later");
    const claimed = (await runCommand(["git", "rev-parse", "HEAD"], tmpDir)).stdout;
    await runCommand(["git", "reset", "--hard", anchor], tmpDir);
    await commitFile("head.txt", "head\n", "new head");

    const result = await new ShellGitAnchorAdapter().collectTouchedFiles({
      repoRoot: tmpDir,
      claimedAtCommit: claimed,
      rebaseFallback: "best-effort",
    });

    expect(result.anchorFallback).toBe("reflog");
    expect(result.notes).toContain("git reflog");
  });

  it("falls back to merge-base for non-reflog side commits", async () => {
    await commitFile("base.txt", "base\n", "base");
    const base = (await runCommand(["git", "rev-parse", "HEAD"], tmpDir)).stdout;

    await Bun.write(join(tmpDir, "side.txt"), "side\n");
    await runCommand(["git", "add", "side.txt"], tmpDir);
    const tree = (await runCommand(["git", "write-tree"], tmpDir)).stdout;
    const sideCommit = (await runCommand(["git", "commit-tree", tree, "-p", base, "-m", "side"], tmpDir)).stdout;
    await runCommand(["git", "reset", "--hard", "HEAD"], tmpDir);
    await commitFile("main.txt", "main\n", "main");

    const result = await new ShellGitAnchorAdapter().collectTouchedFiles({
      repoRoot: tmpDir,
      claimedAtCommit: sideCommit,
      rebaseFallback: "best-effort",
    });

    expect(result.anchorFallback).toBe("merge-base");
    expect(result.notes).toContain(`merge-base ${base}`);
    expect(result.actualFilesTouched).toContain("main.txt");
  });

  it("annotates files introduced through merge commits in the contract window", async () => {
    await commitFile("base.txt", "base\n", "base");
    const anchor = (await runCommand(["git", "rev-parse", "HEAD"], tmpDir)).stdout;

    await runCommand(["git", "checkout", "-b", "feature"], tmpDir);
    await commitFile("feature.txt", "feature\n", "feature work");

    await runCommand(["git", "checkout", "main"], tmpDir);
    await commitFile("main.txt", "main\n", "main work");
    const merged = await runCommand(["git", "merge", "--no-ff", "feature", "-m", "merge feature"], tmpDir);
    expect(merged.exitCode).toBe(0);

    const result = await new ShellGitAnchorAdapter().collectTouchedFiles({
      repoRoot: tmpDir,
      claimedAtCommit: anchor,
      rebaseFallback: "best-effort",
    });

    expect(result.actualFilesTouched).toContain("feature.txt");
    expect(result.notes).toContain("Merge-sourced files:");
    expect(result.notes).toContain("feature.txt");
  });

  it("includes untracked files when collecting touched files for contract verdicts", async () => {
    await commitFile("base.txt", "base\n", "base");
    await Bun.write(join(tmpDir, "scratch.txt"), "scratch\n");

    const result = await new ShellGitAnchorAdapter().collectTouchedFiles({
      repoRoot: tmpDir,
      claimedAtCommit: (await runCommand(["git", "rev-parse", "HEAD"], tmpDir)).stdout,
      rebaseFallback: "best-effort",
    });

    expect(result.actualFilesTouched).toContain("scratch.txt");
    expect(result.notes ?? "").toContain("Includes untracked working-tree files.");
  });

  it("ignores Maestro task runtime files in the touched set", async () => {
    await commitFile("base.txt", "base\n", "base");
    await Bun.write(join(tmpDir, ".maestro", "tasks", "tasks.jsonl"), "{}\n");

    const result = await new ShellGitAnchorAdapter().collectTouchedFiles({
      repoRoot: tmpDir,
      claimedAtCommit: (await runCommand(["git", "rev-parse", "HEAD"], tmpDir)).stdout,
      rebaseFallback: "best-effort",
    });

    expect(result.actualFilesTouched).not.toContain(".maestro/tasks/tasks.jsonl");
  });

  it("keeps repo-tracked contract templates in the touched set", async () => {
    await mkdir(join(tmpDir, ".maestro", "tasks", "contract-templates"), { recursive: true });
    await Bun.write(join(tmpDir, ".maestro", "tasks", "contract-templates", "default.md"), "base\n");
    await runCommand(["git", "add", ".maestro/tasks/contract-templates/default.md"], tmpDir);
    await runCommand(["git", "commit", "-m", "seed contract template"], tmpDir);
    await Bun.write(join(tmpDir, ".maestro", "tasks", "contract-templates", "default.md"), "updated\n");

    const result = await new ShellGitAnchorAdapter().collectTouchedFiles({
      repoRoot: tmpDir,
      claimedAtCommit: (await runCommand(["git", "rev-parse", "HEAD"], tmpDir)).stdout,
      rebaseFallback: "best-effort",
    });

    expect(result.actualFilesTouched).toContain(".maestro/tasks/contract-templates/default.md");
  });
});
