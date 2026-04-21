import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
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

    const drafted = await runCli(["task", "contract", "new", task.id, "--from", templatePath, "--json"], tmpDir);
    const contract = expectJson<{ id: string }>(drafted);
    await runCli(["task", "contract", "lock", contract.id, "--json"], tmpDir);

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

    await runCli(["task", "contract", "new", task.id, "--from", templatePath, "--json"], tmpDir);
    await runCli(["task", "contract", "lock", task.id, "--json"], tmpDir);

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
      await runCli(["task", "contract", "new", firstTask.id, "--from", firstTemplatePath, "--json"], tmpDir),
    );
    await runCli(["task", "contract", "lock", firstContract.id, "--json"], tmpDir);

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
      await runCli(["task", "contract", "new", secondTask.id, "--from", secondTemplatePath, "--json"], tmpDir),
    );
    await runCli(["task", "contract", "lock", secondContract.id, "--json"], tmpDir);

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

    const drafted = await runCli(["task", "contract", "new", task.id, "--from", templatePath, "--json"], tmpDir);
    const contract = expectJson<{ id: string }>(drafted);
    await runCli(["task", "contract", "lock", contract.id, "--json"], tmpDir);
    await runCli(["task", "contract", "criteria", "add", contract.id, "extra check", "--json"], tmpDir);

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
    expect(reset.status).toBe("locked");
    expect(reset.amendments).toHaveLength(1);
    expect(reset.verdict).toBeUndefined();
    expect(reset.closedAt).toBeUndefined();
    expect(reset.closedBy).toBeUndefined();
  }, SLOW_CLI_TIMEOUT_MS);
});
