import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { basename, join } from "node:path";
import { tmpdir } from "node:os";
import { ShellGitAdapter } from "@/infra/adapters/git.adapter.js";
import { runCommand } from "../../../helpers/command-runner.js";

const git = new ShellGitAdapter();
const cwd = process.cwd();
let tempRepo: string;

beforeEach(async () => {
  tempRepo = await mkdtemp(join(tmpdir(), "maestro-git-adapter-"));
  await runCommand(["git", "init", "-b", "main"], tempRepo);
  await runCommand(["git", "config", "user.name", "Test User"], tempRepo);
  await runCommand(["git", "config", "user.email", "test@example.com"], tempRepo);
  await Bun.write(join(tempRepo, "README.md"), "# temp\n");
  await runCommand(["git", "add", "README.md"], tempRepo);
  await runCommand(["git", "commit", "-m", "init"], tempRepo);
});

afterEach(async () => {
  await rm(tempRepo, { recursive: true, force: true });
});

describe("ShellGitAdapter", () => {
  describe("isRepo", () => {
    it("returns true for a git repository", async () => {
      const result = await git.isRepo(cwd);
      expect(result).toBe(true);
    });

    it("returns false for a non-repo directory", async () => {
      const result = await git.isRepo(tmpdir());
      expect(result).toBe(false);
    });
  });

  describe("getState", () => {
    it("returns branch name", async () => {
      const state = await git.getState(cwd);
      expect(state.branch).toBeTruthy();
      expect(typeof state.branch).toBe("string");
    });

    it("returns recent commits as array", async () => {
      const state = await git.getState(cwd);
      expect(Array.isArray(state.recentCommits)).toBe(true);
      expect(state.recentCommits.length).toBeGreaterThan(0);
    });

    it("returns changedFiles as array", async () => {
      const state = await git.getState(cwd);
      expect(Array.isArray(state.changedFiles)).toBe(true);
    });

    it("does not truncate package.json when it appears as the first status line", async () => {
      // Regression for the `ackage.json` bug: execSpawn trims leading whitespace on
      // the full stdout, so a porcelain line like ` M package.json` arrives as
      // `M package.json` when it's the first line. A naive `slice(3)` then eats
      // the leading `p`. The fix is a format-aware parser that accepts either
      // leading-space or trimmed-first-line shapes.
      await Bun.write(join(tempRepo, "package.json"), '{"name":"x"}\n');
      const state = await git.getState(tempRepo);
      expect(state.changedFiles).toContain("package.json");
      expect(state.changedFiles).not.toContain("ackage.json");
    });

    it("returns the renamed path rather than `orig -> new` for rename entries", async () => {
      await Bun.write(join(tempRepo, "old-name.txt"), "hi\n");
      await runCommand(["git", "add", "old-name.txt"], tempRepo);
      await runCommand(["git", "commit", "-m", "add file"], tempRepo);
      await runCommand(["git", "mv", "old-name.txt", "new-name.txt"], tempRepo);

      const state = await git.getState(tempRepo);
      expect(state.changedFiles).toContain("new-name.txt");
      expect(state.changedFiles).not.toContain("old-name.txt -> new-name.txt");
    });

    it("returns workingTreeClean as boolean", async () => {
      const state = await git.getState(cwd);
      expect(typeof state.workingTreeClean).toBe("boolean");
    });

    it("returns diffStat with +/- format", async () => {
      const state = await git.getState(cwd);
      expect(state.diffStat).toMatch(/^\+\d+ -\d+$/);
    });
  });

  describe("getCurrentBranch", () => {
    it("returns the current branch name", async () => {
      expect(await git.getCurrentBranch(tempRepo)).toBe("main");
    });
  });

  describe("createWorktree", () => {
    it("creates a sibling worktree with a codex-prefixed branch", async () => {
      const worktree = await git.createWorktree(tempRepo, {
        slug: "replace-handoff",
        baseBranch: "main",
        branchPrefix: "codex",
      });

      expect(worktree.branch).toBe("codex/replace-handoff");
      expect(worktree.path.endsWith(`${basename(tempRepo)}-replace-handoff`)).toBe(true);
    });

    it("resolves the sibling worktree from the repo root even when called from a nested cwd", async () => {
      const nestedDir = join(tempRepo, "nested", "deeper");
      await mkdir(nestedDir, { recursive: true });

      const worktree = await git.createWorktree(nestedDir, {
        slug: "nested-review",
        baseBranch: "main",
        branchPrefix: "claude",
      });

      expect(worktree.branch).toBe("claude/nested-review");
      expect(worktree.path.endsWith(`${basename(tempRepo)}-nested-review`)).toBe(true);
    });
  });
});
