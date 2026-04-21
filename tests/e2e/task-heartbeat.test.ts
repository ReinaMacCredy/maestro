import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  expectJson,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-heartbeat-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("task heartbeat and stale-claim auto-release", () => {
  it(
    "heartbeat bumps lastActivityAt on a claimed task",
    async () => {
      const created = await runCompiled(
        ["task", "create", "long-running work", "--silent"],
        tmpDir,
      );
      const id = created.stdout;

      await runCompiled(
        ["task", "claim", id, "--session", "operator-live"],
        tmpDir,
      );

      const first = await runCompiled(
        ["task", "show", id, "--json"],
        tmpDir,
      );
      const firstTask = expectJson<{ lastActivityAt?: string }>(first);
      const firstActivity = firstTask.lastActivityAt;
      expect(firstActivity).toBeDefined();

      await new Promise((r) => setTimeout(r, 5));
      await runCompiled(
        ["task", "heartbeat", id, "--session", "operator-live"],
        tmpDir,
      );

      const second = await runCompiled(["task", "show", id, "--json"], tmpDir);
      const secondTask = expectJson<{ lastActivityAt?: string }>(second);
      expect(secondTask.lastActivityAt).not.toBe(firstActivity);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "claim --stale-after releases a stale claim when the owner session cannot be verified",
    async () => {
      const created = await runCompiled(
        ["task", "create", "orphan work", "--silent"],
        tmpDir,
      );
      const id = created.stdout;

      const unknownOwner = "codex-bogusxyz000";
      await runCompiled(
        ["task", "claim", id, "--session", unknownOwner],
        tmpDir,
      );

      await new Promise((r) => setTimeout(r, 20));
      const claim = await runCompiled(
        ["task", "claim", id, "--session", "operator-new", "--stale-after", "1ms", "--json"],
        tmpDir,
      );
      const result = expectJson<{ assignee: string }>(claim);
      expect(result.assignee).toBe("operator-new");
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
