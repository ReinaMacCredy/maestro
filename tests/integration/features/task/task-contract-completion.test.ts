import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expectJson, initGitRepo } from "../../../helpers/run-compiled-cli.js";
import { runCommand } from "../../../helpers/command-runner.js";
import { runCli } from "../../../helpers/run-cli.js";

const SLOW_CLI_TIMEOUT_MS = 30_000;

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-contract-completion-"));
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

describe("task contract completion", () => {
  it("captures claim anchors and stores a fulfilled verdict on completion", async () => {
    await seedTrackedFile("README.md", "hello\n");

    const created = await runCli(["task", "create", "contracted completion", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(created);

    await runCli(["task", "claim", task.id, "--session", "test-owner", "--json"], tmpDir);
    const claimed = await runCli(["task", "show", task.id, "--json"], tmpDir);
    expect(expectJson<{ claimedAtCommit?: string }>(claimed).claimedAtCommit).toMatch(/^[0-9a-f]{40}$/);

    await runCli(["task", "update", task.id, "--status", "in_progress", "--session", "test-owner", "--json"], tmpDir);

    const templatePath = await writeTemplate(
      "completion-template.yaml",
      [
        "intent: Keep the completion scoped to README",
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
      ["task", "contract", "new", task.id, "--from", templatePath, "--session", "test-owner", "--json"],
      tmpDir,
    );
    const contract = expectJson<{ id: string }>(drafted);
    await runCli(["task", "contract", "lock", contract.id, "--session", "test-owner", "--json"], tmpDir);
    await rm(templatePath, { force: true });

    await Bun.write(join(tmpDir, "README.md"), "hello\nworld\n");
    const completed = await runCli(
      [
        "task",
        "update",
        task.id,
        "--status",
        "completed",
        "--reason",
        "done",
        "--summary",
        "updated readme",
        "--verified-by",
        "manual",
        "--session",
        "test-owner",
        "--json",
      ],
      tmpDir,
    );
    expect(expectJson<{ status: string }>(completed).status).toBe("completed");

    const shown = await runCli(["task", "contract", "show", contract.id, "--json"], tmpDir);
    const closed = expectJson<{
      status: string;
      verdict?: {
        fulfilled: boolean;
        actualFilesTouched: string[];
        metCriteria: Array<{ metEvidence?: string }>;
      };
    }>(shown);
    expect(closed.status).toBe("fulfilled");
    expect(closed.verdict?.fulfilled).toBe(true);
    expect(closed.verdict?.actualFilesTouched).toContain("README.md");
    expect(closed.verdict?.metCriteria[0]?.metEvidence).toBe("receipt.verifiedBy:manual");
  }, SLOW_CLI_TIMEOUT_MS);

  it("blocks broken contracted completion in strict mode", async () => {
    await seedTrackedFile("README.md", "hello\n");

    const created = await runCli(["task", "create", "strict completion", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(created);

    await runCli(["task", "claim", task.id, "--session", "strict-owner", "--json"], tmpDir);
    await runCli(["task", "update", task.id, "--status", "in_progress", "--session", "strict-owner", "--json"], tmpDir);

    const templatePath = await writeTemplate(
      "strict-template.yaml",
      [
        "intent: Scope the work away from README",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: manual",
        "    kind: receipt-hint",
        "",
      ].join("\n"),
    );

    await runCli(
      ["task", "contract", "new", task.id, "--from", templatePath, "--session", "strict-owner", "--json"],
      tmpDir,
    );
    await runCli(["task", "contract", "lock", task.id, "--session", "strict-owner", "--json"], tmpDir);
    await rm(templatePath, { force: true });

    await Bun.write(join(tmpDir, "README.md"), "hello\nstrict\n");
    const blocked = await runCli(
      [
        "task",
        "update",
        task.id,
        "--status",
        "completed",
        "--reason",
        "nope",
        "--verified-by",
        "manual",
        "--strict",
        "--session",
        "strict-owner",
      ],
      tmpDir,
    );
    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain("strict mode refused completion");

    const shown = await runCli(["task", "show", task.id, "--json"], tmpDir);
    expect(expectJson<{ status: string }>(shown).status).toBe("in_progress");
  }, SLOW_CLI_TIMEOUT_MS);

  it("previews overlap in verdict output when annotate policy allows concurrent contracts", async () => {
    await seedTrackedFile("README.md", "hello\n");
    await Bun.write(
      join(tmpDir, ".maestro", "config.yaml"),
      "contracts:\n  overlapPolicy: annotate\n",
    );

    const firstTask = expectJson<{ id: string }>(await runCli(["task", "create", "first overlap task", "--json"], tmpDir));
    await runCli(["task", "claim", firstTask.id, "--session", "overlap-owner-1", "--json"], tmpDir);
    await runCli(["task", "update", firstTask.id, "--status", "in_progress", "--session", "overlap-owner-1", "--json"], tmpDir);

    const firstTemplatePath = await writeTemplate(
      "overlap-template-1.yaml",
      [
        "intent: Keep the first overlap task inside README",
        "scope:",
        "  filesExpected:",
        "    - README.md",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: manual",
        "    kind: manual",
        "",
      ].join("\n"),
    );
    const firstContract = expectJson<{ id: string }>(
      await runCli(
        ["task", "contract", "new", firstTask.id, "--from", firstTemplatePath, "--session", "overlap-owner-1", "--json"],
        tmpDir,
      ),
    );
    await runCli(["task", "contract", "lock", firstContract.id, "--session", "overlap-owner-1", "--json"], tmpDir);
    await rm(firstTemplatePath, { force: true });

    const secondTask = expectJson<{ id: string }>(await runCli(["task", "create", "second overlap task", "--json"], tmpDir));
    await runCli(["task", "claim", secondTask.id, "--session", "overlap-owner-2", "--json"], tmpDir);
    await runCli(["task", "update", secondTask.id, "--status", "in_progress", "--session", "overlap-owner-2", "--json"], tmpDir);

    const secondTemplatePath = await writeTemplate(
      "overlap-template-2.yaml",
      [
        "intent: Keep the second overlap task inside README",
        "scope:",
        "  filesExpected:",
        "    - README.md",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: manual",
        "    kind: manual",
        "",
      ].join("\n"),
    );
    const secondContract = expectJson<{ id: string }>(
      await runCli(
        ["task", "contract", "new", secondTask.id, "--from", secondTemplatePath, "--session", "overlap-owner-2", "--json"],
        tmpDir,
      ),
    );
    await runCli(["task", "contract", "lock", secondContract.id, "--session", "overlap-owner-2", "--json"], tmpDir);
    await rm(secondTemplatePath, { force: true });

    await Bun.write(join(tmpDir, "README.md"), "hello\noverlap\n");

    const preview = expectJson<{
      contractId: string;
      verdict: {
        actualFilesTouched: string[];
        overlapDetected?: {
          policy: "fail" | "annotate";
          otherContractIds: string[];
        };
      };
    }>(await runCli(["task", "contract", "verdict", secondContract.id, "--json"], tmpDir));

    expect(preview.contractId).toBe(secondContract.id);
    expect(preview.verdict.actualFilesTouched).toContain("README.md");
    expect(preview.verdict.overlapDetected).toEqual({
      policy: "annotate",
      otherContractIds: [firstContract.id],
    });
  }, SLOW_CLI_TIMEOUT_MS);

  it("includes repo-tracked contract templates in verdict touched files", async () => {
    await mkdir(join(tmpDir, ".maestro", "tasks", "contract-templates"), { recursive: true });
    await Bun.write(
      join(tmpDir, ".maestro", "tasks", "contract-templates", "default.md"),
      "base template\n",
    );
    await runCommand(["git", "config", "user.email", "test@example.com"], tmpDir);
    await runCommand(["git", "config", "user.name", "Test User"], tmpDir);
    await runCommand(["git", "add", ".maestro/tasks/contract-templates/default.md"], tmpDir);
    await runCommand(["git", "commit", "-m", "seed contract template"], tmpDir);

    const created = await runCli(["task", "create", "template verdict scope", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(created);

    await runCli(["task", "claim", task.id, "--session", "template-owner", "--json"], tmpDir);
    await runCli(["task", "update", task.id, "--status", "in_progress", "--session", "template-owner", "--json"], tmpDir);

    const templatePath = await writeTemplate(
      "template-scope.yaml",
      [
        "intent: Keep the work inside repo-tracked contract templates",
        "scope:",
        "  filesExpected:",
        "    - .maestro/tasks/contract-templates/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: template touched",
        "    kind: receipt-hint",
        "",
      ].join("\n"),
    );

    const drafted = await runCli(
      ["task", "contract", "new", task.id, "--from", templatePath, "--session", "template-owner", "--json"],
      tmpDir,
    );
    const contract = expectJson<{ id: string }>(drafted);
    await runCli(["task", "contract", "lock", contract.id, "--session", "template-owner", "--json"], tmpDir);
    await rm(templatePath, { force: true });

    await Bun.write(join(tmpDir, ".maestro", "tasks", "contract-templates", "default.md"), "updated template\n");
    await runCli(
      [
        "task",
        "update",
        task.id,
        "--status",
        "completed",
        "--reason",
        "done",
        "--verified-by",
        "template touched",
        "--session",
        "template-owner",
        "--json",
      ],
      tmpDir,
    );

    const shown = await runCli(["task", "contract", "show", contract.id, "--json"], tmpDir);
    const closed = expectJson<{
      status: string;
      verdict?: {
        fulfilled: boolean;
        actualFilesTouched: string[];
      };
    }>(shown);
    expect(closed.status).toBe("fulfilled");
    expect(closed.verdict?.fulfilled).toBe(true);
    expect(closed.verdict?.actualFilesTouched).toContain(".maestro/tasks/contract-templates/default.md");
  }, SLOW_CLI_TIMEOUT_MS);

  it("includes untracked working-tree files in the stored verdict", async () => {
    await seedTrackedFile("README.md", "hello\n");

    const created = await runCli(["task", "create", "untracked verdict scope", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(created);

    await runCli(["task", "claim", task.id, "--session", "untracked-owner", "--json"], tmpDir);
    await runCli(["task", "update", task.id, "--status", "in_progress", "--session", "untracked-owner", "--json"], tmpDir);

    const templatePath = await writeTemplate(
      "untracked-scope.yaml",
      [
        "intent: Keep the work scoped to an untracked file",
        "scope:",
        "  filesExpected:",
        "    - new-file.txt",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: manual",
        "    kind: receipt-hint",
        "",
      ].join("\n"),
    );

    const drafted = await runCli(
      ["task", "contract", "new", task.id, "--from", templatePath, "--session", "untracked-owner", "--json"],
      tmpDir,
    );
    const contract = expectJson<{ id: string }>(drafted);
    await runCli(["task", "contract", "lock", contract.id, "--session", "untracked-owner", "--json"], tmpDir);
    await rm(templatePath, { force: true });

    await Bun.write(join(tmpDir, "new-file.txt"), "hello from untracked\n");
    await runCli(
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
        "untracked-owner",
        "--json",
      ],
      tmpDir,
    );

    const shown = await runCli(["task", "contract", "show", contract.id, "--json"], tmpDir);
    const closed = expectJson<{
      status: string;
      verdict?: {
        fulfilled: boolean;
        actualFilesTouched: string[];
        filesExpectedUnused: string[];
      };
    }>(shown);
    expect(closed.status).toBe("fulfilled");
    expect(closed.verdict?.fulfilled).toBe(true);
    expect(closed.verdict?.actualFilesTouched).toContain("new-file.txt");
    expect(closed.verdict?.filesExpectedUnused).toEqual([]);
  }, SLOW_CLI_TIMEOUT_MS);

  it("requires a contract only when config asks for it and honors --no-contract", async () => {
    const created = await runCli(["task", "create", "required contract", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(created);
    await runCli(["task", "claim", task.id, "--session", "required-owner", "--json"], tmpDir);
    await runCli(["task", "update", task.id, "--status", "in_progress", "--session", "required-owner", "--json"], tmpDir);

    await Bun.write(
      join(tmpDir, ".maestro", "config.yaml"),
      "contracts:\n  default: required\n",
    );

    const blocked = await runCli(
      ["task", "update", task.id, "--status", "completed", "--reason", "done", "--session", "required-owner"],
      tmpDir,
    );
    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain("requires a locked contract before completion");

    const allowed = await runCli(
      [
        "task",
        "update",
        task.id,
        "--status",
        "completed",
        "--reason",
        "done",
        "--session",
        "required-owner",
        "--no-contract",
        "--json",
      ],
      tmpDir,
    );
    expect(expectJson<{ status: string }>(allowed).status).toBe("completed");
  }, SLOW_CLI_TIMEOUT_MS);

  it("relocks a completed contract when the task is reopened", async () => {
    await seedTrackedFile("README.md", "hello\n");

    const created = await runCli(["task", "create", "reopen contracted task", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(created);

    await runCli(["task", "claim", task.id, "--session", "reopen-owner", "--json"], tmpDir);
    await runCli(["task", "update", task.id, "--status", "in_progress", "--session", "reopen-owner", "--json"], tmpDir);

    const templatePath = await writeTemplate(
      "reopen-template.yaml",
      [
        "intent: Keep the completion scoped to README",
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
    await rm(templatePath, { force: true });
    await runCli(["task", "contract", "criteria", "add", contract.id, "extra check", "--session", "reopen-owner", "--json"], tmpDir);

    await Bun.write(join(tmpDir, "README.md"), "hello\nworld\n");
    await runCli(
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
        "reopen-owner",
        "--json",
      ],
      tmpDir,
    );

    const reopened = await runCli(["task", "reopen", task.id, "--json"], tmpDir);
    expect(expectJson<{ status: string }>(reopened).status).toBe("pending");

    const shown = await runCli(["task", "contract", "show", contract.id, "--json"], tmpDir);
    const reset = expectJson<{
      status: string;
      verdict?: unknown;
      closedAt?: string;
      closedBy?: string;
      amendments: Array<unknown>;
    }>(shown);
    expect(reset.status).toBe("amended");
    expect(reset.amendments).toHaveLength(1);
    expect(reset.verdict).toBeUndefined();
    expect(reset.closedAt).toBeUndefined();
    expect(reset.closedBy).toBeUndefined();
  }, SLOW_CLI_TIMEOUT_MS);

  it("refuses reopen when another active contract already owns the repo and leaves state unchanged", async () => {
    await seedTrackedFile("README.md", "hello\n");

    const closedTask = expectJson<{ id: string }>(await runCli(["task", "create", "closed reopen conflict", "--json"], tmpDir));
    await runCli(["task", "claim", closedTask.id, "--session", "closed-owner", "--json"], tmpDir);
    await runCli(["task", "update", closedTask.id, "--status", "in_progress", "--session", "closed-owner", "--json"], tmpDir);

    const closedTemplatePath = await writeTemplate(
      "closed-reopen-conflict.yaml",
      [
        "intent: Close one contracted task before opening another",
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

    const closedContract = expectJson<{ id: string }>(
      await runCli(
        ["task", "contract", "new", closedTask.id, "--from", closedTemplatePath, "--session", "closed-owner", "--json"],
        tmpDir,
      ),
    );
    await runCli(["task", "contract", "lock", closedContract.id, "--session", "closed-owner", "--json"], tmpDir);
    await rm(closedTemplatePath, { force: true });

    await Bun.write(join(tmpDir, "README.md"), "hello\nclosed\n");
    await runCli(
      [
        "task",
        "update",
        closedTask.id,
        "--status",
        "completed",
        "--reason",
        "done",
        "--verified-by",
        "manual",
        "--session",
        "closed-owner",
        "--json",
      ],
      tmpDir,
    );

    const activeTask = expectJson<{ id: string }>(await runCli(["task", "create", "active reopen conflict", "--json"], tmpDir));
    await runCli(["task", "claim", activeTask.id, "--session", "active-owner", "--json"], tmpDir);
    await runCli(["task", "update", activeTask.id, "--status", "in_progress", "--session", "active-owner", "--json"], tmpDir);

    const activeTemplatePath = await writeTemplate(
      "active-reopen-conflict.yaml",
      [
        "intent: Hold an active lock in the same repo",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: active lock remains",
        "    kind: manual",
        "",
      ].join("\n"),
    );

    await runCli(
      ["task", "contract", "new", activeTask.id, "--from", activeTemplatePath, "--session", "active-owner", "--json"],
      tmpDir,
    );
    await runCli(["task", "contract", "lock", activeTask.id, "--session", "active-owner", "--json"], tmpDir);
    await rm(activeTemplatePath, { force: true });

    const blocked = await runCli(["task", "reopen", closedTask.id, "--json"], tmpDir);
    const blockedOutput = `${blocked.stdout}\n${blocked.stderr}`;
    expect(blocked.exitCode).not.toBe(0);
    expect(blockedOutput).toContain("overlaps an active contract in the same repo");

    const shownTask = await runCli(["task", "show", closedTask.id, "--json"], tmpDir);
    expect(expectJson<{ status: string }>(shownTask).status).toBe("completed");

    const shownContract = await runCli(["task", "contract", "show", closedContract.id, "--json"], tmpDir);
    expect(expectJson<{ status: string }>(shownContract).status).toBe("fulfilled");
  }, SLOW_CLI_TIMEOUT_MS);

  it("reopens a contracted task through update and treats repeated completion as idempotent", async () => {
    await seedTrackedFile("README.md", "hello\n");

    const created = await runCli(["task", "create", "update reopen contracted task", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(created);

    await runCli(["task", "claim", task.id, "--session", "update-reopen-owner", "--json"], tmpDir);
    await runCli(["task", "update", task.id, "--status", "in_progress", "--session", "update-reopen-owner", "--json"], tmpDir);

    const templatePath = await writeTemplate(
      "update-reopen-template.yaml",
      [
        "intent: Keep the update-reopen flow scoped to README",
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
      ["task", "contract", "new", task.id, "--from", templatePath, "--session", "update-reopen-owner", "--json"],
      tmpDir,
    );
    const contract = expectJson<{ id: string }>(drafted);
    await runCli(["task", "contract", "lock", contract.id, "--session", "update-reopen-owner", "--json"], tmpDir);
    await rm(templatePath, { force: true });

    await Bun.write(join(tmpDir, "README.md"), "hello\nfirst close\n");
    await runCli(
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
        "update-reopen-owner",
        "--json",
      ],
      tmpDir,
    );

    const repeated = await runCli(
      [
        "task",
        "update",
        task.id,
        "--status",
        "completed",
        "--session",
        "update-reopen-owner",
        "--json",
      ],
      tmpDir,
    );
    expect(expectJson<{ status: string }>(repeated).status).toBe("completed");

    const rejectedReceiptEdit = await runCli(
      [
        "task",
        "update",
        task.id,
        "--status",
        "completed",
        "--summary",
        "changed after close",
        "--session",
        "update-reopen-owner",
      ],
      tmpDir,
    );
    expect(rejectedReceiptEdit.exitCode).toBe(1);
    expect(rejectedReceiptEdit.stderr).toContain("already completed and cannot be updated");
    expect(rejectedReceiptEdit.stderr).toContain("Reopen the task first");

    const reopened = await runCli(
      ["task", "update", task.id, "--status", "in_progress", "--session", "update-reopen-owner", "--json"],
      tmpDir,
    );
    expect(expectJson<{ status: string; assignee?: string }>(reopened)).toEqual(
      expect.objectContaining({
        status: "in_progress",
        assignee: "update-reopen-owner",
      }),
    );

    const shown = await runCli(["task", "contract", "show", contract.id, "--json"], tmpDir);
    const reset = expectJson<{
      status: string;
      verdict?: unknown;
      closedAtCommit?: string;
      closedBy?: string;
    }>(shown);
    expect(reset.status).toBe("locked");
    expect(reset.verdict).toBeUndefined();
    expect(reset.closedAtCommit).toBeUndefined();
    expect(reset.closedBy).toBeUndefined();
  }, SLOW_CLI_TIMEOUT_MS);

  it("leaves completed task state unchanged when update-reopen cannot claim ownership", async () => {
    await seedTrackedFile("README.md", "hello\n");

    const created = await runCli(["task", "create", "failed update reopen stays closed", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(created);

    await runCli(["task", "claim", task.id, "--session", "restart-owner", "--json"], tmpDir);
    await runCli(["task", "update", task.id, "--status", "in_progress", "--session", "restart-owner", "--json"], tmpDir);

    const templatePath = await writeTemplate(
      "failed-reopen-template.yaml",
      [
        "intent: Keep the completion scoped to README",
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
      ["task", "contract", "new", task.id, "--from", templatePath, "--session", "restart-owner", "--json"],
      tmpDir,
    );
    const contract = expectJson<{ id: string }>(drafted);
    await runCli(["task", "contract", "lock", contract.id, "--session", "restart-owner", "--json"], tmpDir);
    await rm(templatePath, { force: true });

    await Bun.write(join(tmpDir, "README.md"), "hello\nrestart\n");
    await runCli(
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
        "restart-owner",
        "--json",
      ],
      tmpDir,
    );

    const failed = await runCli(
      ["task", "update", task.id, "--status", "in_progress"],
      tmpDir,
      {
        env: {
          CLAUDECODE: "0",
          CODEX_THREAD_ID: "",
        },
      },
    );
    expect(failed.exitCode).toBe(1);
    expect(failed.stderr).toContain("Status 'in_progress' requires task ownership");

    const shownTask = await runCli(["task", "show", task.id, "--json"], tmpDir);
    expect(expectJson<{ status: string }>(shownTask).status).toBe("completed");

    const shownContract = await runCli(["task", "contract", "show", contract.id, "--json"], tmpDir);
    const closed = expectJson<{
      status: string;
      verdict?: { fulfilled: boolean };
    }>(shownContract);
    expect(closed.status).toBe("fulfilled");
    expect(closed.verdict?.fulfilled).toBe(true);
  }, SLOW_CLI_TIMEOUT_MS);
});
