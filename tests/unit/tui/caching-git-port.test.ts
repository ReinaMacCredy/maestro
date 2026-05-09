import { describe, it, expect } from "bun:test";
import type { GitPort } from "@/infra/ports/git.port.js";
import type { GitState, GitWorktree } from "@/infra/domain/git-types.js";
import { CachingGitPort } from "@/tui/state/snapshot-poll-cache.js";

describe("CachingGitPort", () => {
  it("implements all GitPort methods", () => {
    const mockInner: GitPort = {
      getState: async () => ({
        branch: "main",
        recentCommits: [],
        changedFiles: [],
        fileChanges: [],
        workingTreeClean: true,
        diffStat: "+0 -0",
      }),
      isRepo: async () => true,
      getCurrentBranch: async () => "main",
      createWorktree: async () => ({
        slug: "test",
        baseBranch: "main",
        branch: "feat/test",
        path: "/tmp/test",
      }),
    };

    const cache = new CachingGitPort(mockInner);

    // Check that all required methods exist
    expect(typeof cache.getState).toBe("function");
    expect(typeof cache.isRepo).toBe("function");
    expect(typeof cache.getCurrentBranch).toBe("function");
    expect(typeof cache.createWorktree).toBe("function");
  });

  it("delegates getCurrentBranch to inner port", async () => {
    let called = false;
    const mockInner: GitPort = {
      getState: async () => ({
        branch: "main",
        recentCommits: [],
        changedFiles: [],
        fileChanges: [],
        workingTreeClean: true,
        diffStat: "+0 -0",
      }),
      isRepo: async () => true,
      getCurrentBranch: async (cwd: string) => {
        called = true;
        expect(cwd).toBe("/test");
        return "feature-branch";
      },
      createWorktree: async () => ({
        slug: "test",
        baseBranch: "main",
        branch: "feat/test",
        path: "/tmp/test",
      }),
    };

    const cache = new CachingGitPort(mockInner);
    const result = await cache.getCurrentBranch("/test");

    expect(called).toBe(true);
    expect(result).toBe("feature-branch");
  });

  it("delegates createWorktree to inner port", async () => {
    let called = false;
    const mockInner: GitPort = {
      getState: async () => ({
        branch: "main",
        recentCommits: [],
        changedFiles: [],
        fileChanges: [],
        workingTreeClean: true,
        diffStat: "+0 -0",
      }),
      isRepo: async () => true,
      getCurrentBranch: async () => "main",
      createWorktree: async (cwd: string, input) => {
        called = true;
        expect(cwd).toBe("/test");
        expect(input.slug).toBe("my-feature");
        return {
          slug: input.slug,
          baseBranch: input.baseBranch,
          branch: `${input.branchPrefix}/${input.slug}`,
          path: `/tmp/test-${input.slug}`,
        };
      },
    };

    const cache = new CachingGitPort(mockInner);
    const result = await cache.createWorktree("/test", {
      slug: "my-feature",
      baseBranch: "main",
      branchPrefix: "feat",
    });

    expect(called).toBe(true);
    expect(result.slug).toBe("my-feature");
    expect(result.branch).toBe("feat/my-feature");
  });
});
