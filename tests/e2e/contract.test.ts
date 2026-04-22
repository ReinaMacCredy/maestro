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

async function writeEditorScript(name: string, replacement: string): Promise<string> {
  const path = join(tmpDir, `${name.replace(/\.sh$/, "")}.ts`);
  await Bun.write(
    path,
    `await Bun.write(process.argv[2] ?? "", ${JSON.stringify(replacement)});\n`,
  );
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
      (
        await runCompiled(
          ["task", "contract", "new", task.id, "--from", templatePath, "--session", "compiled-owner", "--json"],
          tmpDir,
        )
      ).stdout,
    ) as { id: string };
    await runCompiled(["task", "contract", "lock", contract.id, "--session", "compiled-owner", "--json"], tmpDir);
    await rm(templatePath, { force: true });

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

  it("previews the current verdict through the compiled CLI", async () => {
    await seedTrackedFile("README.md", "hello\n");

    const task = JSON.parse((await runCompiled(["task", "create", "compiled preview", "--json"], tmpDir)).stdout) as {
      id: string;
    };
    await runCompiled(["task", "claim", task.id, "--session", "compiled-preview-owner", "--json"], tmpDir);
    await runCompiled(
      ["task", "update", task.id, "--status", "in_progress", "--session", "compiled-preview-owner", "--json"],
      tmpDir,
    );

    const templatePath = await writeTemplate(
      "preview-template.yaml",
      [
        "intent: Preview the verdict before completion",
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

    const contract = JSON.parse(
      (
        await runCompiled(
          [
            "task",
            "contract",
            "new",
            task.id,
            "--from",
            templatePath,
            "--session",
            "compiled-preview-owner",
            "--json",
          ],
          tmpDir,
        )
      ).stdout,
    ) as { id: string };
    await runCompiled(
      ["task", "contract", "lock", contract.id, "--session", "compiled-preview-owner", "--json"],
      tmpDir,
    );

    await Bun.write(join(tmpDir, "README.md"), "hello\npreview\n");

    const preview = JSON.parse(
      (await runCompiled(["task", "contract", "verdict", contract.id, "--json"], tmpDir)).stdout,
    ) as {
      contractId: string;
      verdict: {
        fulfilled: boolean;
        actualFilesTouched: string[];
      };
    };
    expect(preview.contractId).toBe(contract.id);
    expect(preview.verdict.fulfilled).toBe(false);
    expect(preview.verdict.actualFilesTouched).toContain("README.md");
  }, SLOW_CLI_TIMEOUT_MS);

  it("prints the plain ok marker for amend and criteria mutators in silent mode", async () => {
    const taskId = (await runCompiled(["task", "create", "silent amend", "--silent"], tmpDir)).stdout;
    await runCompiled(["task", "claim", taskId, "--session", "compiled-silent-owner", "--json"], tmpDir);
    await runCompiled(
      ["task", "update", taskId, "--status", "in_progress", "--session", "compiled-silent-owner", "--json"],
      tmpDir,
    );
    const templatePath = await writeTemplate(
      "silent-amend-template.yaml",
      [
        "intent: Keep contract work in task files",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: criterion exists",
        "    kind: manual",
        "",
      ].join("\n"),
    );

    const created = await runCompiled(
      ["task", "contract", "new", taskId, "--from", templatePath, "--session", "compiled-silent-owner", "--json"],
      tmpDir,
    );
    const contract = JSON.parse(created.stdout) as {
      id: string;
      doneWhen: Array<{ id: string }>;
    };
    await runCompiled(
      ["task", "contract", "lock", contract.id, "--session", "compiled-silent-owner", "--json"],
      tmpDir,
    );

    const editorPath = await writeEditorScript(
      "silent-amend-editor.sh",
      [
        "intent: Keep contract work in task files and tests",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "    - tests/e2e/**",
        "  filesForbidden: []",
        "doneWhen:",
        `  - id: ${contract.doneWhen[0]?.id}`,
        "    text: criterion exists in source and e2e",
        "    kind: manual",
        "",
      ].join("\n"),
    );

    const amended = await runCompiled(
      [
        "task",
        "contract",
        "amend",
        contract.id,
        "--reason",
        "expand checks",
        "--session",
        "compiled-silent-owner",
        "--silent",
      ],
      tmpDir,
      { env: { EDITOR: `bun '${editorPath}'` } },
    );
    expect(amended.stdout).toBe(`${contract.id} [ok]`);

    const added = await runCompiled(
      [
        "task",
        "contract",
        "criteria",
        "add",
        contract.id,
        "extra criterion",
        "--session",
        "compiled-silent-owner",
        "--silent",
      ],
      tmpDir,
    );
    expect(added.stdout).toBe(`${contract.id} [ok]`);

    const shown = JSON.parse((await runCompiled(["task", "contract", "show", contract.id, "--json"], tmpDir)).stdout) as {
      doneWhen: Array<{ id: string; text: string }>;
    };
    const addedCriterion = shown.doneWhen.find((criterion) => criterion.text === "extra criterion");
    expect(addedCriterion?.id).toMatch(/^dw-[0-9a-f]{6}$/);

    const marked = await runCompiled(
      [
        "task",
        "contract",
        "criteria",
        "mark",
        contract.id,
        addedCriterion!.id,
        "--met",
        "--session",
        "compiled-silent-owner",
        "--silent",
      ],
      tmpDir,
    );
    expect(marked.stdout).toBe(`${contract.id} [ok]`);

    const removed = await runCompiled(
      [
        "task",
        "contract",
        "criteria",
        "remove",
        contract.id,
        addedCriterion!.id,
        "--session",
        "compiled-silent-owner",
        "--silent",
      ],
      tmpDir,
    );
    expect(removed.stdout).toBe(`${contract.id} [ok]`);
  }, SLOW_CLI_TIMEOUT_MS);

  it("reactivates amended contracts after task reopen", async () => {
    await seedTrackedFile("README.md", "hello\n");

    const task = JSON.parse((await runCompiled(["task", "create", "compiled reopen", "--json"], tmpDir)).stdout) as {
      id: string;
    };
    await runCompiled(["task", "claim", task.id, "--session", "compiled-owner", "--json"], tmpDir);
    await runCompiled(
      ["task", "update", task.id, "--status", "in_progress", "--session", "compiled-owner", "--json"],
      tmpDir,
    );

    const templatePath = await writeTemplate(
      "reopen-verdict-template.yaml",
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
      (
        await runCompiled(
          ["task", "contract", "new", task.id, "--from", templatePath, "--session", "compiled-owner", "--json"],
          tmpDir,
        )
      ).stdout,
    ) as { id: string };
    await runCompiled(["task", "contract", "lock", contract.id, "--session", "compiled-owner", "--json"], tmpDir);
    await rm(templatePath, { force: true });
    await runCompiled(
      ["task", "contract", "criteria", "add", contract.id, "extra check", "--session", "compiled-owner", "--json"],
      tmpDir,
    );

    await Bun.write(join(tmpDir, "README.md"), "hello\ncompiled\n");
    await runCompiled(
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

    const reopened = await runCompiled(["task", "reopen", task.id, "--json"], tmpDir);
    expect(JSON.parse(reopened.stdout)).toEqual(expect.objectContaining({ status: "pending" }));

    const shown = JSON.parse(
      (await runCompiled(["task", "contract", "show", contract.id, "--json"], tmpDir)).stdout,
    ) as {
      status: string;
      verdict?: unknown;
      amendments: unknown[];
    };
    expect(shown.status).toBe("amended");
    expect(shown.amendments).toHaveLength(1);
    expect(shown.verdict).toBeUndefined();
  }, SLOW_CLI_TIMEOUT_MS);
});
