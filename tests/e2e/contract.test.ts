import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";
import { runCommand } from "../helpers/command-runner.js";

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-contract-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function writeTemplate(name: string, body: string): Promise<string> {
  const path = join(tmpDir, name);
  await Bun.write(path, body);
  return path;
}

async function seedTrackedFile(path: string, content: string): Promise<void> {
  await Bun.write(join(tmpDir, path), content);
  await runCommand(["git", "config", "user.email", "test@example.com"], tmpDir);
  await runCommand(["git", "config", "user.name", "Test User"], tmpDir);
  await runCommand(["git", "add", path], tmpDir);
  await runCommand(["git", "commit", "-m", "seed tracked file"], tmpDir);
}

describe("task contract compiled E2E", () => {
  it("prints the plain ok marker for new and lock in silent mode", async () => {
    const taskId = (await runCompiled(["task", "create", "silent contract", "--silent"], tmpDir)).stdout;
    const templatePath = await writeTemplate(
      "contract-template.yaml",
      [
        "intent: silent contract flow",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: silent mode works",
        "",
      ].join("\n"),
    );

    const drafted = await runCompiled(
      ["task", "contract", "new", taskId, "--from", templatePath, "--silent"],
      tmpDir,
    );
    expect(drafted.stdout).toMatch(/^c-[0-9a-f]{6} \[ok\]$/);

    const contractId = drafted.stdout.split(" ")[0]!;
    const locked = await runCompiled(
      ["task", "contract", "lock", contractId, "--silent"],
      tmpDir,
    );
    expect(locked.stdout).toBe(`${contractId} [ok]`);
  }, SLOW_CLI_TIMEOUT_MS);

  it("prints the plain ok marker for discard in silent mode", async () => {
    const taskId = (await runCompiled(["task", "create", "discard silent contract", "--silent"], tmpDir)).stdout;
    const templatePath = await writeTemplate(
      "discard-template.yaml",
      [
        "intent: discard this draft",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: discard works",
        "",
      ].join("\n"),
    );

    const drafted = await runCompiled(
      ["task", "contract", "new", taskId, "--from", templatePath, "--silent"],
      tmpDir,
    );
    const contractId = drafted.stdout.split(" ")[0]!;

    const discarded = await runCompiled(
      ["task", "contract", "discard", contractId, "--silent"],
      tmpDir,
    );
    expect(discarded.stdout).toBe(`${contractId} [ok]`);
  }, SLOW_CLI_TIMEOUT_MS);

  it("stores a fulfilled verdict after compiled completion", async () => {
    await seedTrackedFile("README.md", "hello\n");

    const task = JSON.parse((await runCompiled(["task", "create", "compiled verdict", "--json"], tmpDir)).stdout) as {
      id: string;
    };
    await runCompiled(["task", "claim", task.id, "--session", "compiled-owner", "--json"], tmpDir);
    await runCompiled(
      ["task", "update", task.id, "--status", "in_progress", "--session", "compiled-owner", "--json"],
      tmpDir,
    );

    const templatePath = await writeTemplate(
      "verdict-template.yaml",
      [
        "intent: Keep the compiled completion inside README",
        "scope:",
        "  filesExpected:",
        "    - README.md",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: manual",
        "    kind: receipt-hint",
        "",
      ].join("\n"),
    );

    const contract = JSON.parse(
      (await runCompiled(["task", "contract", "new", task.id, "--from", templatePath, "--json"], tmpDir)).stdout,
    ) as { id: string };
    await runCompiled(["task", "contract", "lock", contract.id, "--json"], tmpDir);

    await Bun.write(join(tmpDir, "README.md"), "hello\ncompiled\n");
    const completed = await runCompiled(
      [
        "task",
        "update",
        task.id,
        "--status",
        "completed",
        "--reason",
        "done",
        "--verified-by",
        "manual",
        "--session",
        "compiled-owner",
        "--json",
      ],
      tmpDir,
    );
    expect(JSON.parse(completed.stdout)).toEqual(expect.objectContaining({ status: "completed" }));

    const shown = JSON.parse(
      (await runCompiled(["task", "contract", "show", contract.id, "--json"], tmpDir)).stdout,
    ) as {
      status: string;
      verdict?: { fulfilled: boolean; actualFilesTouched: string[] };
    };
    expect(shown.status).toBe("fulfilled");
    expect(shown.verdict?.fulfilled).toBe(true);
    expect(shown.verdict?.actualFilesTouched).toContain("README.md");
  }, SLOW_CLI_TIMEOUT_MS);
});
