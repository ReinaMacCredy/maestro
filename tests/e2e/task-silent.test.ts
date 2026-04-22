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

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-silent-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("task mutating commands --silent", () => {
  it(
    "prints '<id> <marker>' for update/claim/unclaim/block/unblock/reopen",
    async () => {
      const first = await runCompiled(["task", "create", "first", "--silent"], tmpDir);
      const id = first.stdout;
      expect(id).toMatch(/^tsk-[0-9a-f]{6}$/);

      const claimResult = await runCompiled(
        ["task", "claim", id, "--session", "operator-a", "--silent"],
        tmpDir,
      );
      expect(claimResult.stdout).toBe(`${id} .`);

      const updateResult = await runCompiled(
        ["task", "update", id, "--status", "in_progress", "--session", "operator-a", "--silent"],
        tmpDir,
      );
      expect(updateResult.stdout).toBe(`${id} >`);

      const completeResult = await runCompiled(
        [
          "task",
          "update",
          id,
          "--status",
          "completed",
          "--session",
          "operator-a",
          "--reason",
          "done",
          "--silent",
        ],
        tmpDir,
      );
      expect(completeResult.stdout).toBe(`${id} x`);

      const reopenResult = await runCompiled(
        ["task", "reopen", id, "--silent"],
        tmpDir,
      );
      expect(reopenResult.stdout).toBe(`${id} .`);

      const second = await runCompiled(["task", "create", "second", "--silent"], tmpDir);
      const id2 = second.stdout;
      const blockResult = await runCompiled(
        ["task", "block", id, id2, "--silent"],
        tmpDir,
      );
      expect(blockResult.stdout).toBe(`${id} .`);

      const unblockResult = await runCompiled(
        ["task", "unblock", id, id2, "--silent"],
        tmpDir,
      );
      expect(unblockResult.stdout).toBe(`${id} .`);

      await runCompiled(
        ["task", "claim", id, "--session", "operator-a", "--silent"],
        tmpDir,
      );
      const unclaimResult = await runCompiled(
        ["task", "unclaim", id, "--session", "operator-a"],
        tmpDir,
        { env: { MAESTRO_TASK_SILENT: "1" } },
      );
      expect(unclaimResult.stdout).toBe(`${id} .`);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "respects MAESTRO_TASK_SILENT=1 without --silent flag",
    async () => {
      const first = await runCompiled(["task", "create", "env-silent", "--silent"], tmpDir);
      const id = first.stdout;

      const result = await runCompiled(
        ["task", "claim", id, "--session", "operator-b"],
        tmpDir,
        { env: { MAESTRO_TASK_SILENT: "1" } },
      );
      expect(result.stdout).toBe(`${id} .`);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "respects MAESTRO_TASK_SILENT=TRUE without --silent flag",
    async () => {
      const first = await runCompiled(["task", "create", "env-silent-uppercase", "--silent"], tmpDir);
      const id = first.stdout;

      const result = await runCompiled(
        ["task", "claim", id, "--session", "operator-c"],
        tmpDir,
        { env: { MAESTRO_TASK_SILENT: "TRUE" } },
      );
      expect(result.stdout).toBe(`${id} .`);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "silent mode falls back to verbose output on failure",
    async () => {
      const bogus = await runCompiled(
        ["task", "claim", "tsk-zzzzzz", "--session", "x", "--silent"],
        tmpDir,
      );
      expect(bogus.exitCode).not.toBe(0);
      expect(bogus.stderr.length).toBeGreaterThan(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
