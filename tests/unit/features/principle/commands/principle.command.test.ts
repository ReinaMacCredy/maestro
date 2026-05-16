import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { initGitRepo } from "../../../../helpers/command-runner.js";
import { runCli } from "../../../../helpers/run-cli.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-principle-command-"));
  await initGitRepo(tmpDir);
  const initResult = await runCli(["init", "--json"], tmpDir);
  expect(initResult.exitCode).toBe(0);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("principle command", () => {
  it("lists bootstrapped default principles (v1 jsonl store starts empty on v2 init)", async () => {
    // v2 init seeds docs/principles/<slug>.md, not .maestro/principles.jsonl.
    // The v1 `principle list` command reads from principles.jsonl which is
    // empty on a fresh v2 project -- correct behaviour.
    const result = await runCli(["principle", "list", "--json"], tmpDir);
    expect(result.exitCode).toBe(0);

    const principles = JSON.parse(result.stdout) as Array<{ id: string }>;
    expect(principles).toHaveLength(0);
  });

  it("adds a principle and filters it by profile", async () => {
    const addResult = await runCli([
      "principle",
      "add",
      "--id", "test-review-principle",
      "--name", "Test Review Principle",
      "--rule", "Review carefully",
      "--profiles", "code-review",
      "--mode", "advisory",
      "--json",
    ], tmpDir);
    expect(addResult.exitCode).toBe(0);

    const listResult = await runCli([
      "principle",
      "list",
      "--profile", "code-review",
      "--json",
    ], tmpDir);
    expect(listResult.exitCode).toBe(0);

    const principles = JSON.parse(listResult.stdout) as Array<{ id: string }>;
    expect(principles.map((principle) => principle.id)).toContain("test-review-principle");
  });

  it("rejects invalid profile filters", async () => {
    const result = await runCli([
      "principle",
      "list",
      "--profile", "definitely-not-a-profile",
      "--json",
    ], tmpDir);

    expect(result.exitCode).toBe(1);
    expect(result.stdout).toContain("Invalid principle profile");
  });

  it("removes an added principle", async () => {
    const addResult = await runCli([
      "principle",
      "add",
      "--id", "temporary-principle",
      "--name", "Temporary Principle",
      "--rule", "Temporary rule",
      "--profiles", "planning",
      "--mode", "advisory",
      "--json",
    ], tmpDir);
    expect(addResult.exitCode).toBe(0);

    const removeResult = await runCli([
      "principle",
      "remove",
      "temporary-principle",
      "--json",
    ], tmpDir);
    expect(removeResult.exitCode).toBe(0);

    const listResult = await runCli(["principle", "list", "--json"], tmpDir);
    const principles = JSON.parse(listResult.stdout) as Array<{ id: string }>;
    expect(principles.map((principle) => principle.id)).not.toContain("temporary-principle");
  });

  it("rejects invalid gate principles without gate metadata", async () => {
    const result = await runCli([
      "principle",
      "add",
      "--id", "broken-gate",
      "--name", "Broken Gate",
      "--rule", "Missing metadata",
      "--profiles", "implementation",
      "--mode", "gate",
    ], tmpDir);

    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("Invalid principle");
    expect(result.stderr).toContain("Gate-mode principles require --gate-field and --gate-check");
  });
});
