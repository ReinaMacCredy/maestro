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

async function writeEditorScript(name: string, replacement: string): Promise<string> {
  const path = join(tmpDir, name);
  await Bun.write(
    path,
    [
      "#!/bin/sh",
      "cat <<'EOF' > \"$1\"",
      replacement,
      "EOF",
      "",
    ].join("\n"),
  );
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

  it("amends a locked contract and manages criteria", async () => {
    const createdTask = await runCli(["task", "create", "criteria contract", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(createdTask);
    const templatePath = await writeTemplate(
      "criteria-template.yaml",
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
    const contract = expectJson<{ id: string; doneWhen: Array<{ id: string }> }>(drafted);
    await runCli(["task", "contract", "lock", contract.id, "--json"], tmpDir);

    const editorPath = await writeEditorScript(
      "amend-editor.sh",
      [
        "intent: Keep the task work and tests inside the task surface",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "    - tests/integration/features/task/**",
        "  filesForbidden:",
        "    - src/features/mission/**",
        "doneWhen:",
        `  - id: ${contract.doneWhen[0]?.id}`,
        "    text: task contract commands cover source and tests",
        "    kind: manual",
        "",
      ].join("\n"),
    );

    const amended = await runCli(
      ["task", "contract", "amend", contract.id, "--reason", "expanded test coverage", "--json"],
      tmpDir,
      { env: { EDITOR: `sh ${editorPath}` } },
    );
    const amendedContract = expectJson<{ status: string; scope: { filesExpected: string[] }; amendments: Array<{ reason: string }> }>(amended);
    expect(amendedContract.status).toBe("amended");
    expect(amendedContract.scope.filesExpected).toContain("tests/integration/features/task/**");
    expect(amendedContract.amendments.at(-1)?.reason).toBe("expanded test coverage");

    const added = await runCli(
      ["task", "contract", "criteria", "add", contract.id, "receipt hint exists", "--json"],
      tmpDir,
    );
    const addedContract = expectJson<{ doneWhen: Array<{ id: string; text: string }>; amendments: Array<{ reason: string }> }>(added);
    const addedCriterion = addedContract.doneWhen.find((criterion) => criterion.text === "receipt hint exists");
    expect(addedCriterion?.id).toMatch(/^dw-[0-9a-f]{6}$/);
    expect(addedContract.amendments.at(-1)?.reason).toContain("Added criterion");

    const marked = await runCli(
      [
        "task",
        "contract",
        "criteria",
        "mark",
        contract.id,
        addedCriterion!.id,
        "--met",
        "--evidence",
        "manual",
        "--json",
      ],
      tmpDir,
    );
    const markedContract = expectJson<{ doneWhen: Array<{ id: string; met?: boolean; metEvidence?: string }> }>(marked);
    expect(markedContract.doneWhen.find((criterion) => criterion.id === addedCriterion!.id)).toEqual(
      expect.objectContaining({
        id: addedCriterion!.id,
        met: true,
        metEvidence: "manual",
      }),
    );

    const removed = await runCli(
      ["task", "contract", "criteria", "remove", contract.id, addedCriterion!.id, "--json"],
      tmpDir,
    );
    expect(expectJson<{ doneWhen: Array<{ id: string }> }>(removed).doneWhen.map((criterion) => criterion.id)).toEqual([
      contract.doneWhen[0]!.id,
    ]);
  }, SLOW_CLI_TIMEOUT_MS);
});
