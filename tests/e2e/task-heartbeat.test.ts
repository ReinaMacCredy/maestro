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

async function writeTemplate(name: string, body: string): Promise<string> {
  const path = join(tmpDir, name);
  await Bun.write(path, body);
  return path;
}

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

  it(
    "stale reclaim inherits contract ownership by default and blocks when configured",
    async () => {
      const created = await runCompiled(["task", "create", "stale contract", "--json"], tmpDir);
      const task = expectJson<{ id: string }>(created);
      const staleOwner = "codex-bogusxyz001";

      await runCompiled(["task", "claim", task.id, "--session", staleOwner, "--json"], tmpDir);
      await runCompiled(["task", "update", task.id, "--status", "in_progress", "--session", staleOwner, "--json"], tmpDir);

      const templatePath = await writeTemplate(
        "stale-contract-template.yaml",
        [
          "intent: Keep stale reclaim inside task files",
          "scope:",
          "  filesExpected:",
          "    - src/features/task/**",
          "  filesForbidden: []",
          "doneWhen:",
          "  - text: contract exists",
          "",
        ].join("\n"),
      );
      await runCompiled(
        ["task", "contract", "new", task.id, "--from", templatePath, "--session", staleOwner, "--json"],
        tmpDir,
      );
      await runCompiled(["task", "contract", "lock", task.id, "--session", staleOwner, "--json"], tmpDir);

      await new Promise((r) => setTimeout(r, 20));
      const claimed = await runCompiled(
        ["task", "claim", task.id, "--session", "operator-new", "--stale-after", "1ms", "--json"],
        tmpDir,
      );
      expect(expectJson<{ assignee: string }>(claimed).assignee).toBe("operator-new");

      const inherited = await runCompiled(["task", "contract", "show", task.id, "--json"], tmpDir);
      expect(expectJson<{ lockedBy?: string }>(inherited).lockedBy).toBe("operator-new");

      const blockedCreated = await runCompiled(["task", "create", "blocked stale contract", "--json"], tmpDir);
      const blockedTask = expectJson<{ id: string }>(blockedCreated);
      const blockedOwner = "codex-bogusxyz002";

      await runCompiled(["task", "claim", blockedTask.id, "--session", blockedOwner, "--json"], tmpDir);
      await runCompiled(
        ["task", "update", blockedTask.id, "--status", "in_progress", "--session", blockedOwner, "--json"],
        tmpDir,
      );
      await Bun.write(
        join(tmpDir, ".maestro", "config.yaml"),
        "contracts:\n  staleReclaimContractPolicy: block\n  overlapPolicy: annotate\n",
      );
      await runCompiled(
        ["task", "contract", "new", blockedTask.id, "--from", templatePath, "--session", blockedOwner, "--json"],
        tmpDir,
      );
      const locked = await runCompiled(
        ["task", "contract", "lock", blockedTask.id, "--session", blockedOwner, "--json"],
        tmpDir,
      );
      expect(expectJson<{ status: string }>(locked).status).toBe("locked");

      await new Promise((r) => setTimeout(r, 20));
      const blocked = await runCompiled(
        ["task", "claim", blockedTask.id, "--session", "operator-blocked", "--stale-after", "1ms"],
        tmpDir,
      );
      expect(blocked.exitCode).not.toBe(0);
      expect(blocked.stderr).toContain("stale reclaim is blocked by contract policy");
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
