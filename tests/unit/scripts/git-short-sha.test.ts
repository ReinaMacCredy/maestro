import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { getGitShortSha } from "../../../scripts/git-short-sha";
import { runCommand } from "../../helpers/command-runner.js";

let tmpDir: string;

describe("getGitShortSha", () => {
  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-git-sha-"));
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
  });

  it("returns the current short HEAD sha inside a git repository", async () => {
    await runCommand(["git", "init", "-b", "main"], tmpDir);
    await runCommand(["git", "config", "user.name", "Maestro Tests"], tmpDir);
    await runCommand(["git", "config", "user.email", "tests@example.com"], tmpDir);
    await writeFile(join(tmpDir, "README.md"), "hello\n");
    await runCommand(["git", "add", "README.md"], tmpDir);
    await runCommand(["git", "commit", "-m", "test"], tmpDir);

    const sha = await getGitShortSha(tmpDir);

    expect(sha).toMatch(/^[a-f0-9]{7}$/);
  });

  it("returns undefined when the directory is not a git repository", async () => {
    expect(await getGitShortSha(tmpDir)).toBeUndefined();
  });
});
