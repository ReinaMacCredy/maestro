import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
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
      expect(worktree.path).toBe(join(dirname(tempRepo), `${basename(tempRepo)}-replace-handoff`));
    });
  });
});
