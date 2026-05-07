import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runCommand } from "@/../tests/helpers/command-runner";
import { resolveDefaultBase } from "@/shared/lib/git-base.js";

const EMPTY_TREE_SHA = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

let tmpDir: string;
let originalCwd: string;

async function initRepo(branch: string): Promise<void> {
  await runCommand(["git", "init", "-b", branch], tmpDir);
  await runCommand(["git", "config", "user.email", "test@example.com"], tmpDir);
  await runCommand(["git", "config", "user.name", "Test"], tmpDir);
  await runCommand(["git", "commit", "--allow-empty", "-m", "init"], tmpDir);
}

beforeEach(async () => {
  originalCwd = process.cwd();
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-git-base-"));
});

afterEach(async () => {
  process.chdir(originalCwd);
  await rm(tmpDir, { recursive: true, force: true });
});

describe("resolveDefaultBase", () => {
  it("falls back to the empty-tree SHA when on a master branch with no main/trunk", async () => {
    await initRepo("master");
    process.chdir(tmpDir);

    const base = await resolveDefaultBase();
    expect(base).toBe(EMPTY_TREE_SHA);
  });

  it("falls back to merge-base with master when only master exists", async () => {
    await initRepo("master");
    // Create a feature branch on top so merge-base is meaningful.
    await runCommand(["git", "checkout", "-b", "feature"], tmpDir);
    await runCommand(["git", "commit", "--allow-empty", "-m", "feature commit"], tmpDir);
    process.chdir(tmpDir);

    const base = await resolveDefaultBase();
    expect(base).not.toBe(EMPTY_TREE_SHA);
    expect(base).toMatch(/^[0-9a-f]{40}$/);
  });

  it("returns merge-base with main when on a main-defaulting repo", async () => {
    await initRepo("main");
    await runCommand(["git", "checkout", "-b", "feature"], tmpDir);
    await runCommand(["git", "commit", "--allow-empty", "-m", "feature commit"], tmpDir);
    process.chdir(tmpDir);

    const base = await resolveDefaultBase();
    expect(base).toMatch(/^[0-9a-f]{40}$/);
  });
});
