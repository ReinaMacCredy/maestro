import { describe, expect, it } from "bun:test";
import { mkdtemp, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  createWorktreeForTask,
  formatCreateWorktreeLines,
} from "@/features/worktree/usecases/create-worktree.usecase.js";
import type { GitPort } from "@/infra/ports/git.port.js";
import type { GitWorktree } from "@/infra/domain/git-types.js";

function makeFakeGit(worktreePath: string): GitPort {
  return {
    async getState() {
      return {
        branch: "main",
        recentCommits: [],
        changedFiles: [],
        workingTreeClean: true,
        diffStat: "",
      };
    },
    async isRepo() {
      return true;
    },
    async getCurrentBranch() {
      return "main";
    },
    async createWorktree(_cwd, input): Promise<GitWorktree> {
      return {
        slug: input.slug,
        baseBranch: input.baseBranch,
        branch: `${input.branchPrefix}/${input.slug}`,
        path: worktreePath,
      };
    },
  };
}

describe("createWorktreeForTask", () => {
  it("creates an isolated .maestro/runs directory inside the new worktree", async () => {
    const worktreePath = await mkdtemp(join(tmpdir(), "wt-"));
    try {
      const git = makeFakeGit(worktreePath);
      const r = await createWorktreeForTask(
        { git },
        { cwd: process.cwd(), slug: "harness-pivot" },
      );
      expect(r.worktree.slug).toBe("harness-pivot");
      expect(r.worktree.branch).toBe("feat/harness-pivot");
      const s = await stat(r.runsDir);
      expect(s.isDirectory()).toBe(true);
    } finally {
      await rm(worktreePath, { recursive: true, force: true });
    }
  });

  it("uses --base and --prefix overrides", async () => {
    const worktreePath = await mkdtemp(join(tmpdir(), "wt-"));
    try {
      const git = makeFakeGit(worktreePath);
      const r = await createWorktreeForTask(
        { git },
        { cwd: process.cwd(), slug: "x", baseBranch: "develop", branchPrefix: "fix" },
      );
      expect(r.worktree.baseBranch).toBe("develop");
      expect(r.worktree.branch).toBe("fix/x");
    } finally {
      await rm(worktreePath, { recursive: true, force: true });
    }
  });

  it("formats output with helpful next-step hint", async () => {
    const worktreePath = await mkdtemp(join(tmpdir(), "wt-"));
    try {
      const git = makeFakeGit(worktreePath);
      const r = await createWorktreeForTask({ git }, { cwd: process.cwd(), slug: "x" });
      const lines = formatCreateWorktreeLines(r);
      expect(lines.some((l) => l.startsWith("Created worktree:"))).toBe(true);
      expect(lines.some((l) => l.startsWith("Next:"))).toBe(true);
    } finally {
      await rm(worktreePath, { recursive: true, force: true });
    }
  });
});
