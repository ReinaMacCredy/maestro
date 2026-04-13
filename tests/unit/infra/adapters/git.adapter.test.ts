import { describe, expect, it } from "bun:test";
import { ShellGitAdapter } from "@/infra/adapters/git.adapter.js";

const git = new ShellGitAdapter();
const cwd = process.cwd();

describe("ShellGitAdapter", () => {
  describe("isRepo", () => {
    it("returns true for a git repository", async () => {
      const result = await git.isRepo(cwd);
      expect(result).toBe(true);
    });

    it("returns false for a non-repo directory", async () => {
      const result = await git.isRepo("/tmp");
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
});
