import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { initGitRepo } from "../../../helpers/run-compiled-cli.js";
import { runCli } from "../../../helpers/run-cli.js";

const SLOW_CLI_TIMEOUT_MS = 30_000;

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-contract-cli-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

function expectJson<T>(result: { stdout: string }): T {
  return JSON.parse(result.stdout) as T;
}

async function writeTemplate(name: string, body: string): Promise<string> {
  const path = join(tmpDir, name);
  await Bun.write(path, body);
  return path;
}

describe("task contract CLI", () => {
  it("creates, shows, lists, and locks a contract from a template", async () => {
    const createdTask = await runCli(["task", "create", "contracted task", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(createdTask);
    const templatePath = await writeTemplate(
      "contract-template.yaml",
      [
        "intent: Keep the task work inside the task feature",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden:",
        "    - src/features/mission/**",
        "doneWhen:",
        "  - text: task contract commands are available",
        "    kind: manual",
        "",
      ].join("\n"),
    );

    const drafted = await runCli(["task", "contract", "new", task.id, "--from", templatePath, "--json"], tmpDir);
    const contract = expectJson<{ id: string; status: string; taskId: string; doneWhen: Array<{ id: string }> }>(drafted);
    expect(contract.id).toMatch(/^c-[0-9a-f]{6}$/);
    expect(contract.status).toBe("draft");
    expect(contract.taskId).toBe(task.id);
    expect(contract.doneWhen[0]?.id).toMatch(/^dw-[0-9a-f]{6}$/);

    const shownTask = await runCli(["task", "show", task.id, "--json"], tmpDir);
    expect(expectJson<{ contractId?: string }>(shownTask).contractId).toBe(contract.id);

    const shownContract = await runCli(["task", "contract", "show", task.id], tmpDir);
    expect(shownContract.stdout).toContain(contract.id);
    expect(shownContract.stdout).toContain("Status: draft");

    const listed = await runCli(["task", "contract", "list", "--json"], tmpDir);
    expect(expectJson<Array<{ id: string }>>(listed).map((entry) => entry.id)).toContain(contract.id);

    const locked = await runCli(["task", "contract", "lock", contract.id, "--json"], tmpDir);
    const lockedContract = expectJson<{ status: string; claimedAtCommit?: string }>(locked);
    expect(lockedContract.status).toBe("locked");
    expect(lockedContract.claimedAtCommit).toMatch(/^[0-9a-f]{40}$/);
  }, SLOW_CLI_TIMEOUT_MS);

  it("discards a draft contract and filters it by status", async () => {
    const createdTask = await runCli(["task", "create", "discarded contract", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(createdTask);
    const templatePath = await writeTemplate(
      "discard-template.yaml",
      [
        "intent: Throw away this draft",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: draft can be discarded",
        "",
      ].join("\n"),
    );

    const drafted = await runCli(["task", "contract", "new", task.id, "--from", templatePath, "--json"], tmpDir);
    const contract = expectJson<{ id: string }>(drafted);

    const discarded = await runCli(["task", "contract", "discard", contract.id, "--json"], tmpDir);
    expect(expectJson<{ status: string }>(discarded).status).toBe("discarded");

    const listed = await runCli(["task", "contract", "list", "--status", "discarded", "--json"], tmpDir);
    expect(expectJson<Array<{ id: string; status: string }>>(listed)).toEqual([
      expect.objectContaining({ id: contract.id, status: "discarded" }),
    ]);
  }, SLOW_CLI_TIMEOUT_MS);
});
