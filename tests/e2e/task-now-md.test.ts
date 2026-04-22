import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, rm, access } from "node:fs/promises";
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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-now-md-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("task NOW.md recitation", () => {
  it(
    "creates NOW.md on first task create and reflects updates",
    async () => {
      const nowMdPath = join(tmpDir, ".maestro", "tasks", "NOW.md");

      await expect(access(nowMdPath)).rejects.toThrow();

      const created = await runCompiled(
        ["task", "create", "write the auth middleware", "--silent"],
        tmpDir,
      );
      const taskId = created.stdout;
      expect(taskId).toMatch(/^tsk-[0-9a-f]{6}$/);

      const afterCreate = await readFile(nowMdPath, "utf8");
      expect(afterCreate).toContain("# NOW");
      expect(afterCreate).toContain("Updated:");
      expect(afterCreate).toContain(`${taskId} . write the auth middleware`);
      expect(afterCreate).toContain("## Ready to pick up (1)");

      await runCompiled(
        ["task", "update", taskId, "--status", "in_progress", "--session", "operator-a"],
        tmpDir,
      );

      const afterStart = await readFile(nowMdPath, "utf8");
      expect(afterStart).toContain("## In progress (1)");
      expect(afterStart).toContain("Owner: ");
      expect(afterStart).toContain("## Ready to pick up (0)");

      await runCompiled(
        [
          "task",
          "update",
          taskId,
          "--status",
          "completed",
          "--reason",
          "shipped",
          "--session",
          "operator-a",
        ],
        tmpDir,
      );

      const afterComplete = await readFile(nowMdPath, "utf8");
      expect(afterComplete).toContain("## In progress (0)");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "writes 'No tasks yet.' when the store is empty",
    async () => {
      const nowMdPath = join(tmpDir, ".maestro", "tasks", "NOW.md");
      const created = await runCompiled(
        ["task", "create", "temp", "--silent"],
        tmpDir,
      );
      const taskId = created.stdout;

      const mdAfter = await readFile(nowMdPath, "utf8");
      expect(mdAfter).toContain(taskId);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "includes the active contract summary for in-progress work",
    async () => {
      const nowMdPath = join(tmpDir, ".maestro", "tasks", "NOW.md");
      const created = await runCompiled(["task", "create", "contracted now line", "--json"], tmpDir);
      const task = JSON.parse(created.stdout) as { id: string };

      const templatePath = join(tmpDir, "contract-template.yaml");
      await Bun.write(
        templatePath,
        [
          "intent: Keep NOW.md coverage scoped to README",
          "scope:",
          "  filesExpected:",
          "    - README.md",
          "  filesForbidden: []",
          "doneWhen:",
          "  - text: criteria exists",
          "    kind: manual",
          "",
        ].join("\n"),
      );

      const contract = JSON.parse(
        (await runCompiled(
          ["task", "contract", "new", task.id, "--from", templatePath, "--session", "operator-a", "--json"],
          tmpDir,
        )).stdout,
      ) as { id: string };
      await runCompiled(["task", "contract", "lock", contract.id, "--session", "operator-a", "--json"], tmpDir);
      await runCompiled(["task", "claim", task.id, "--session", "operator-a", "--json"], tmpDir);
      await runCompiled(
        ["task", "update", task.id, "--status", "in_progress", "--session", "operator-a", "--json"],
        tmpDir,
      );

      const md = await readFile(nowMdPath, "utf8");
      expect(md).toContain(`Contract: ${contract.id} (locked, 0/1 done-when met, scope: README.md)`);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
