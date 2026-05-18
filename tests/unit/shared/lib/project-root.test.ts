import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";

let tmp: string;

beforeEach(async () => {
  tmp = await mkdtemp(join(tmpdir(), "maestro-project-root-"));
});

afterEach(async () => {
  await rm(tmp, { recursive: true, force: true });
});

describe("resolveMaestroProjectRoot", () => {
  it("returns the main repo root when called from a git worktree", async () => {
    const mainRepo = join(tmp, "repo");
    const worktree = join(tmp, "repo-feature");
    const worktreeGitDir = join(mainRepo, ".git", "worktrees", "repo-feature");

    await mkdir(join(mainRepo, ".maestro"), { recursive: true });
    await mkdir(worktreeGitDir, { recursive: true });
    await mkdir(worktree, { recursive: true });
    await writeFile(join(worktree, ".git"), `gitdir: ${worktreeGitDir}\n`);
    await writeFile(join(worktreeGitDir, "commondir"), "../..\n");

    expect(resolveMaestroProjectRoot(worktree)).toBe(await realpath(mainRepo));
  });

  it("still falls back to the git root when no maestro project root exists", async () => {
    const repo = join(tmp, "plain-repo");

    await mkdir(join(repo, ".git"), { recursive: true });

    expect(resolveMaestroProjectRoot(repo)).toBe(await realpath(repo));
  });

  it("does not walk past the git repo root to an unrelated ancestor .maestro/", async () => {
    // Regression: a stray .maestro/ in an ancestor (e.g. /tmp/.maestro/ from
    // a prior test) was silently adopted as the project root for any non-
    // maestro git repo nested under it. setup verbs reported [ok] while
    // doing nothing in the user's actual repo.
    const ancestor = join(tmp, "ancestor");
    const repo = join(ancestor, "non-maestro-repo");

    await mkdir(join(ancestor, ".maestro"), { recursive: true });
    await mkdir(join(repo, ".git"), { recursive: true });

    expect(resolveMaestroProjectRoot(repo)).toBe(await realpath(repo));
  });

  it("returns the main repo root when the worktree has a tracked local .maestro/", async () => {
    // Reproduces the v0.72.2 demo bug: in a real worktree created from a
    // commit that already had .maestro/ committed, the worktree's checkout
    // contains a tracked .maestro/ snapshot. The resolver must still walk
    // via `.git/commondir` to the main worktree — the local snapshot is
    // never canonical in a linked worktree.
    const mainRepo = join(tmp, "repo");
    const worktree = join(tmp, "repo-feature");
    const worktreeGitDir = join(mainRepo, ".git", "worktrees", "repo-feature");

    await mkdir(join(mainRepo, ".maestro"), { recursive: true });
    await mkdir(join(worktree, ".maestro", "policies"), { recursive: true }); // tracked .maestro/ snapshot
    await mkdir(worktreeGitDir, { recursive: true });
    await writeFile(join(worktree, ".git"), `gitdir: ${worktreeGitDir}\n`);
    await writeFile(join(worktreeGitDir, "commondir"), "../..\n");

    expect(resolveMaestroProjectRoot(worktree)).toBe(await realpath(mainRepo));
  });
});
