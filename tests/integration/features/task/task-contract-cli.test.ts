import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
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
  const path = join(tmpDir, `${name.replace(/\.sh$/, "")}.ts`);
  await Bun.write(
    path,
    `await Bun.write(process.argv[2] ?? "", ${JSON.stringify(replacement)});\n`,
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
    const contract = expectJson<{
      id: string;
      status: string;
      taskId: string;
      repoRoot: string;
      doneWhen: Array<{ id: string }>;
    }>(drafted);
    expect(contract.id).toMatch(/^c-[0-9a-f]{6}$/);
    expect(contract.status).toBe("draft");
    expect(contract.taskId).toBe(task.id);
    expect(contract.repoRoot).toBe(".");
    expect(contract.doneWhen[0]?.id).toMatch(/^dw-[0-9a-f]{6}$/);

    const shownTask = await runCli(["task", "show", task.id, "--json"], tmpDir);
    expect(expectJson<{ contractId?: string }>(shownTask).contractId).toBe(contract.id);

    const shownContract = await runCli(["task", "contract", "show", task.id], tmpDir);
    expect(shownContract.stdout).toContain(contract.id);
    expect(shownContract.stdout).toContain("Status: draft");
    expect(shownContract.stdout).not.toContain(tmpDir);

    const listed = await runCli(["task", "contract", "list", "--json"], tmpDir);
    expect(expectJson<Array<{ id: string }>>(listed).map((entry) => entry.id)).toContain(contract.id);

    const locked = await runCli(["task", "contract", "lock", contract.id, "--json"], tmpDir);
    const lockedContract = expectJson<{ status: string; claimedAtCommit?: string }>(locked);
    expect(lockedContract.status).toBe("locked");
    expect(lockedContract.claimedAtCommit).toMatch(/^[0-9a-f]{40}$/);
  }, SLOW_CLI_TIMEOUT_MS);

  it("loads named templates from .maestro/tasks/contract-templates", async () => {
    const createdTask = await runCli(["task", "create", "templated contract", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(createdTask);
    const templateDir = join(tmpDir, ".maestro", "tasks", "contract-templates");
    const nestedDir = join(tmpDir, "nested", "deeper");
    await mkdir(templateDir, { recursive: true });
    await mkdir(nestedDir, { recursive: true });
    await Bun.write(
      join(templateDir, "default.md"),
      [
        "intent: Create the contract from the repo-local default template",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: named template lookup works",
        "    kind: manual",
        "",
      ].join("\n"),
    );

    const drafted = await runCli(["task", "contract", "new", task.id, "--from", "default", "--json"], nestedDir);
    const contract = expectJson<{ intent: string; scope: { filesExpected: string[] } }>(drafted);
    expect(contract.intent).toBe("Create the contract from the repo-local default template");
    expect(contract.scope.filesExpected).toEqual(["src/features/task/**"]);
  }, SLOW_CLI_TIMEOUT_MS);

  it("warns when forbidden scope overlaps expected scope at lock time", async () => {
    const createdTask = await runCli(["task", "create", "overlapping scope contract", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(createdTask);
    const templatePath = await writeTemplate(
      "overlap-scope-template.yaml",
      [
        "intent: Demonstrate overlapping contract scope",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden:",
        "    - src/features/task/commands/**",
        "doneWhen:",
        "  - text: overlap warning is visible",
        "",
      ].join("\n"),
    );

    const drafted = await runCli(["task", "contract", "new", task.id, "--from", templatePath, "--json"], tmpDir);
    const contract = expectJson<{ id: string }>(drafted);

    const locked = await runCli(["task", "contract", "lock", contract.id, "--json"], tmpDir);
    expect(expectJson<{ status: string }>(locked).status).toBe("locked");
    expect(locked.stderr).toContain("filesForbidden overlaps filesExpected");
    expect(locked.stderr).toContain("src/features/task/commands/**");
  }, SLOW_CLI_TIMEOUT_MS);

  it("requires the owning session to create and lock a contract for a claimed task", async () => {
    const createdTask = await runCli(["task", "create", "claimed contract ownership", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(createdTask);
    await runCli(["task", "claim", task.id, "--session", "owner-a", "--json"], tmpDir);
    await runCli(["task", "update", task.id, "--status", "in_progress", "--session", "owner-a", "--json"], tmpDir);

    const templatePath = await writeTemplate(
      "claimed-contract-template.yaml",
      [
        "intent: Only the owner can establish this contract",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: owner validates the contract",
        "    kind: manual",
        "",
      ].join("\n"),
    );

    const rejectedDraft = await runCli(
      ["task", "contract", "new", task.id, "--from", templatePath, "--session", "owner-b"],
      tmpDir,
    );
    expect(rejectedDraft.exitCode).toBe(1);

    const drafted = await runCli(
      ["task", "contract", "new", task.id, "--from", templatePath, "--session", "owner-a", "--json"],
      tmpDir,
    );
    const contract = expectJson<{ id: string }>(drafted);

    const rejectedLock = await runCli(
      ["task", "contract", "lock", contract.id, "--session", "owner-b"],
      tmpDir,
    );
    expect(rejectedLock.exitCode).toBe(1);

    const locked = await runCli(
      ["task", "contract", "lock", contract.id, "--session", "owner-a", "--json"],
      tmpDir,
    );
    expect(expectJson<{ status: string }>(locked).status).toBe("locked");
  }, SLOW_CLI_TIMEOUT_MS);

  it("snapshots contract config at lock time, not draft time", async () => {
    const createdTask = await runCli(["task", "create", "lock-time config snapshot", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(createdTask);
    const templatePath = await writeTemplate(
      "lock-config-template.yaml",
      [
        "intent: Capture the final lock-time contract policy",
        "scope:",
        "  filesExpected:",
        "    - README.md",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: config is captured at lock",
        "",
      ].join("\n"),
    );

    const drafted = await runCli(["task", "contract", "new", task.id, "--from", templatePath, "--json"], tmpDir);
    const contract = expectJson<{ id: string }>(drafted);

    await Bun.write(
      join(tmpDir, ".maestro", "config.yaml"),
      [
        "contracts:",
        "  strict: true",
        "  overlapPolicy: annotate",
        "  rebaseFallback: fail",
        "  defaultMaxFilesTouched: 1",
        "  staleReclaimContractPolicy: block",
        "",
      ].join("\n"),
    );

    const locked = await runCli(["task", "contract", "lock", contract.id, "--json"], tmpDir);
    const lockedContract = expectJson<{
      configSnapshot: {
        strict: boolean;
        overlapPolicy: string;
        rebaseFallback: string;
        defaultMaxFilesTouched?: number;
        staleReclaimContractPolicy: string;
      };
    }>(locked);

    expect(lockedContract.configSnapshot).toEqual({
      strict: true,
      overlapPolicy: "annotate",
      rebaseFallback: "fail",
      defaultMaxFilesTouched: 1,
      staleReclaimContractPolicy: "block",
    });
  }, SLOW_CLI_TIMEOUT_MS);

  it("uses the lock-time overlap policy when another active contract already exists", async () => {
    const firstTask = expectJson<{ id: string }>(await runCli(["task", "create", "first overlap task", "--json"], tmpDir));
    const secondTask = expectJson<{ id: string }>(await runCli(["task", "create", "second overlap task", "--json"], tmpDir));
    const templatePath = await writeTemplate(
      "annotate-overlap-template.yaml",
      [
        "intent: Allow overlap when config changes before lock",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: overlap policy is evaluated at lock time",
        "",
      ].join("\n"),
    );

    const firstDraft = expectJson<{ id: string }>(
      await runCli(["task", "contract", "new", firstTask.id, "--from", templatePath, "--json"], tmpDir),
    );
    await runCli(["task", "contract", "lock", firstDraft.id, "--json"], tmpDir);

    const secondDraft = expectJson<{ id: string }>(
      await runCli(["task", "contract", "new", secondTask.id, "--from", templatePath, "--json"], tmpDir),
    );

    await Bun.write(
      join(tmpDir, ".maestro", "config.yaml"),
      [
        "contracts:",
        "  overlapPolicy: annotate",
        "",
      ].join("\n"),
    );

    const locked = await runCli(["task", "contract", "lock", secondDraft.id, "--json"], tmpDir);
    expect(expectJson<{ status: string; configSnapshot: { overlapPolicy: string } }>(locked)).toEqual(
      expect.objectContaining({
        status: "locked",
        configSnapshot: expect.objectContaining({ overlapPolicy: "annotate" }),
      }),
    );
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

    const shownTask = await runCli(["task", "show", task.id, "--json"], tmpDir);
    expect(expectJson<{ contractId?: string }>(shownTask).contractId).toBeUndefined();

    const listed = await runCli(["task", "contract", "list", "--status", "discarded", "--json"], tmpDir);
    expect(expectJson<Array<{ id: string; status: string }>>(listed)).toEqual([
      expect.objectContaining({ id: contract.id, status: "discarded" }),
    ]);

    const draftedAgain = await runCli(["task", "contract", "new", task.id, "--from", templatePath, "--json"], tmpDir);
    const second = expectJson<{ id: string; status: string }>(draftedAgain);
    expect(second.id).not.toBe(contract.id);
    expect(second.status).toBe("draft");
  }, SLOW_CLI_TIMEOUT_MS);

  it("respects MAESTRO_TASK_SILENT=TRUE for contract mutators", async () => {
    const createdTask = await runCli(["task", "create", "env silent contract", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(createdTask);
    const templatePath = await writeTemplate(
      "env-silent-contract.yaml",
      [
        "intent: Verify contract silent-mode env parity",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: contract can be discarded silently",
        "",
      ].join("\n"),
    );

    const drafted = await runCli(["task", "contract", "new", task.id, "--from", templatePath, "--json"], tmpDir);
    const contract = expectJson<{ id: string }>(drafted);

    const discarded = await runCli(
      ["task", "contract", "discard", contract.id],
      tmpDir,
      { env: { MAESTRO_TASK_SILENT: "TRUE" } },
    );
    expect(discarded.stdout).toBe(`${contract.id} [ok]`);
  }, SLOW_CLI_TIMEOUT_MS);

  it("amends a locked contract and manages criteria", async () => {
    const createdTask = await runCli(["task", "create", "criteria contract", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(createdTask);
    await runCli(["task", "claim", task.id, "--session", "criteria-owner", "--json"], tmpDir);
    await runCli(["task", "update", task.id, "--status", "in_progress", "--session", "criteria-owner", "--json"], tmpDir);
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

    const drafted = await runCli(
      ["task", "contract", "new", task.id, "--from", templatePath, "--session", "criteria-owner", "--json"],
      tmpDir,
    );
    const contract = expectJson<{ id: string; doneWhen: Array<{ id: string }> }>(drafted);
    await runCli(["task", "contract", "lock", contract.id, "--session", "criteria-owner", "--json"], tmpDir);

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
      ["task", "contract", "amend", contract.id, "--reason", "expanded test coverage", "--session", "criteria-owner", "--json"],
      tmpDir,
      { env: { EDITOR: `bun '${editorPath}'` } },
    );
    const amendedContract = expectJson<{ status: string; scope: { filesExpected: string[] }; amendments: Array<{ reason: string }> }>(amended);
    expect(amendedContract.status).toBe("amended");
    expect(amendedContract.scope.filesExpected).toContain("tests/integration/features/task/**");
    expect(amendedContract.amendments.at(-1)?.reason).toBe("expanded test coverage");

    const added = await runCli(
      ["task", "contract", "criteria", "add", contract.id, "receipt hint exists", "--session", "criteria-owner", "--json"],
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
        "--session",
        "criteria-owner",
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
      ["task", "contract", "criteria", "remove", contract.id, addedCriterion!.id, "--session", "criteria-owner", "--json"],
      tmpDir,
    );
    expect(expectJson<{ doneWhen: Array<{ id: string }> }>(removed).doneWhen.map((criterion) => criterion.id)).toEqual([
      contract.doneWhen[0]!.id,
    ]);
  }, SLOW_CLI_TIMEOUT_MS);

  it("rejects active contract mutations from a different session", async () => {
    const createdTask = await runCli(["task", "create", "owned contract mutation", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(createdTask);
    await runCli(["task", "claim", task.id, "--session", "owner-a", "--json"], tmpDir);
    await runCli(["task", "update", task.id, "--status", "in_progress", "--session", "owner-a", "--json"], tmpDir);

    const templatePath = await writeTemplate(
      "owned-contract-template.yaml",
      [
        "intent: Keep the mutation owned by one session",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: active contract ownership is enforced",
        "    kind: manual",
        "",
      ].join("\n"),
    );

    const drafted = await runCli(
      ["task", "contract", "new", task.id, "--from", templatePath, "--session", "owner-a", "--json"],
      tmpDir,
    );
    const contract = expectJson<{ id: string; status: string }>(drafted);
    await runCli(["task", "contract", "lock", contract.id, "--session", "owner-a", "--json"], tmpDir);

    const rejected = await runCli(
      ["task", "contract", "criteria", "add", contract.id, "unauthorized criterion", "--session", "owner-b"],
      tmpDir,
    );
    expect(rejected.exitCode).toBe(1);
    expect(rejected.stderr).toContain(`Contract ${contract.id} is owned by owner-a`);
    expect(rejected.stderr).toContain("current session cannot modify it");

    const shown = await runCli(["task", "contract", "show", contract.id, "--json"], tmpDir);
    const payload = expectJson<{
      status: string;
      amendments: Array<unknown>;
      doneWhen: Array<{ text: string }>;
    }>(shown);
    expect(payload.status).toBe("locked");
    expect(payload.amendments).toHaveLength(0);
    expect(payload.doneWhen.map((criterion) => criterion.text)).not.toContain("unauthorized criterion");
  }, SLOW_CLI_TIMEOUT_MS);

  it("edits a draft contract before lock", async () => {
    const createdTask = await runCli(["task", "create", "editable contract", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(createdTask);
    const templatePath = await writeTemplate(
      "editable-template.yaml",
      [
        "intent: Initial draft intent",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: initial criterion",
        "",
      ].join("\n"),
    );
    const drafted = await runCli(["task", "contract", "new", task.id, "--from", templatePath, "--json"], tmpDir);
    const contract = expectJson<{ id: string }>(drafted);

    const editorPath = await writeEditorScript(
      "edit-editor.sh",
      [
        "intent: Edited draft intent",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "    - tests/integration/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: edited criterion",
        "    kind: manual",
        "",
      ].join("\n"),
    );

    const edited = await runCli(
      ["task", "contract", "edit", contract.id, "--json"],
      tmpDir,
      { env: { EDITOR: `bun '${editorPath}'` } },
    );
    const payload = expectJson<{ status: string; intent: string; scope: { filesExpected: string[] } }>(edited);
    expect(payload.status).toBe("draft");
    expect(payload.intent).toBe("Edited draft intent");
    expect(payload.scope.filesExpected).toContain("tests/integration/features/task/**");
  }, SLOW_CLI_TIMEOUT_MS);

  it("reopens a completed amended contract through the contract surface and relocks it", async () => {
    await Bun.write(join(tmpDir, "README.md"), "seed\n");
    await runCli(["git", "config", "user.email", "test@example.com"], tmpDir);
    await runCli(["git", "config", "user.name", "Test User"], tmpDir);
    await runCli(["git", "add", "README.md"], tmpDir);
    await runCli(["git", "commit", "-m", "seed"], tmpDir);

    const createdTask = await runCli(["task", "create", "reopen via contract", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(createdTask);
    await runCli(["task", "claim", task.id, "--session", "reopen-owner", "--json"], tmpDir);
    await runCli(["task", "update", task.id, "--status", "in_progress", "--session", "reopen-owner", "--json"], tmpDir);

    const templatePath = await writeTemplate(
      "reopen-via-contract.yaml",
      [
        "intent: Keep reopen flow scoped to README",
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

    const drafted = await runCli(
      ["task", "contract", "new", task.id, "--from", templatePath, "--session", "reopen-owner", "--json"],
      tmpDir,
    );
    const contract = expectJson<{ id: string }>(drafted);
    await runCli(["task", "contract", "lock", contract.id, "--session", "reopen-owner", "--json"], tmpDir);
    await runCli(
      ["task", "contract", "criteria", "add", contract.id, "extra amended criterion", "--session", "reopen-owner", "--json"],
      tmpDir,
    );

    await Bun.write(join(tmpDir, "README.md"), "seed\nupdated\n");
    await runCli(
      ["task", "update", task.id, "--status", "completed", "--reason", "done", "--verified-by", "manual", "--session", "reopen-owner", "--json"],
      tmpDir,
    );

    const reopened = await runCli(["task", "contract", "reopen", contract.id, "--json"], tmpDir);
    expect(expectJson<{ status: string }>(reopened).status).toBe("amended");

    const shownTask = await runCli(["task", "show", task.id, "--json"], tmpDir);
    expect(expectJson<{ status: string }>(shownTask).status).toBe("pending");
  }, SLOW_CLI_TIMEOUT_MS);
});
