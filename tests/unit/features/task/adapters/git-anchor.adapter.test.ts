import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
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
});
