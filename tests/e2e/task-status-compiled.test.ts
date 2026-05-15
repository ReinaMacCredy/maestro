import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-status-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

// TODO(v2/phase-4): re-enable or remove. v1 `task claim` is detached per ADR-0007 big-bang;
// v2 equivalents live in src/v2/runtime/task.command.ts.
describe.skip("compiled task status + plan slug conversion", () => {
  it(
    "renders the screenshot fixture and exposes slugs in JSON / list / show",
    async () => {
      const planPath = join(tmpDir, "plan.json");
      await writeFile(
        planPath,
        JSON.stringify({
          tasks: [
            {
              name: "race",
              title: "Pass git config overrides to prevent .git/config.lock race",
              type: "feature",
              slug: "implement/worktree-config-lock-race",
            },
            {
              name: "prompts",
              title: "Template prompt fixes",
              type: "feature",
              slug: "implement/template-prompt-fixes",
            },
            {
              name: "p1",
              title: "Remove contradictory close-issue instruction from implement-prompt.md",
              parent: "prompts",
            },
            {
              name: "p2",
              title: "Replace hardcoded 'main' in review-prompt.md with {{SOURCE_BRANCH}}",
              parent: "prompts",
            },
            {
              name: "p3",
              title: "Return reviewer result from Phase 2 callback in parallel-planner-with-review",
              parent: "prompts",
            },
            {
              name: "init",
              title: "Init template e2e tests",
              type: "feature",
              slug: "implement/init-template-e2e-tests",
            },
            {
              name: "i1",
              title: "Add AgentInvoker seam, test support module, and blank template e2e test",
              parent: "init",
              blockedBy: ["prompts"],
            },
            { name: "i2", title: "Add e2e test for simple-loop init template", parent: "init" },
            { name: "i3", title: "Add e2e test for sequential-reviewer init template", parent: "init" },
            { name: "i4", title: "Add e2e test for parallel-planner init template", parent: "init" },
            {
              name: "agentErr",
              title: "Agent error text investigation",
              type: "feature",
              slug: "implement/agent-error-text-investigation",
            },
            {
              name: "a1",
              title: "Investigate and surface Pi agent error text on non-zero exit",
              parent: "agentErr",
            },
            {
              name: "a2",
              title: "Investigate and surface Codex agent error text on non-zero exit",
              parent: "agentErr",
            },
            {
              name: "a3",
              title: "Investigate and surface OpenCode agent error text on non-zero exit",
              parent: "agentErr",
            },
          ],
        }),
        "utf8",
      );

      const planResult = await runCompiled(["task", "plan", "--file", planPath], tmpDir);
      expect(planResult.exitCode).toBe(0);

      const list = await runCompiled(["task", "list", "--json"], tmpDir);
      const tasks = expectJson<Array<{ id: string; slug?: string; title: string; parentId?: string }>>(list);
      const worktree = tasks.find((t) => t.slug === "implement/worktree-config-lock-race");
      expect(worktree).toBeDefined();

      // claim + start the worktree task as in_progress
      const claim = await runCompiled(
        ["task", "claim", worktree!.id, "--session", "operator-a"],
        tmpDir,
      );
      expect(claim.exitCode).toBe(0);
      const start = await runCompiled(
        [
          "task",
          "update",
          worktree!.id,
          "--status",
          "in_progress",
          "--session",
          "operator-a",
        ],
        tmpDir,
      );
      expect(start.exitCode).toBe(0);

      // start one prompt step + one agent-err step so the active count reaches 3
      const prompts = tasks.filter((t) => t.parentId !== undefined);
      const promptStep1 = prompts.find((t) =>
        t.title === "Remove contradictory close-issue instruction from implement-prompt.md",
      );
      const agentErrStep1 = prompts.find((t) =>
        t.title === "Investigate and surface Pi agent error text on non-zero exit",
      );

      const startStep = async (taskId: string, sessionId: string) => {
        await runCompiled(["task", "claim", taskId, "--session", sessionId], tmpDir);
        const r = await runCompiled(
          ["task", "update", taskId, "--status", "in_progress", "--session", sessionId, "--force"],
          tmpDir,
        );
        expect(r.exitCode).toBe(0);
      };
      await startStep(promptStep1!.id, "operator-b");
      await startStep(agentErrStep1!.id, "operator-c");

      const status = await runCompiled(["task", "status"], tmpDir, {
        env: { ...process.env, NO_COLOR: "1" },
      });
      expect(status.exitCode).toBe(0);
      expect(status.stdout).toContain(
        "tasks: 12 open | 3 active | 7 ready | 2 blocked | 1 blocked track",
      );
      expect(status.stdout).toContain("implement/worktree-config-lock-race");
      expect(status.stdout).toContain("blocked by implement/template-prompt-fixes");

      const groupedStatus = await runCompiled(["task", "status", "--no-compact"], tmpDir, {
        env: { ...process.env, NO_COLOR: "1" },
      });
      expect(groupedStatus.exitCode).toBe(0);
      expect(groupedStatus.stdout).toContain("tasks: 3 active, 7 pending, 2 blocked");
      expect(groupedStatus.stdout).toContain("      in-progress");

      const showSlug = await runCompiled(
        ["task", "show", "implement/init-template-e2e-tests"],
        tmpDir,
      );
      expect(showSlug.exitCode).toBe(0);
      expect(showSlug.stdout).toContain("Slug: implement/init-template-e2e-tests");

      const tracksOnly = await runCompiled(["task", "list", "--tracks"], tmpDir);
      expect(tracksOnly.stdout.split("\n").sort()).toEqual([
        "implement/agent-error-text-investigation",
        "implement/init-template-e2e-tests",
        "implement/template-prompt-fixes",
        "implement/worktree-config-lock-race",
      ].sort());

      const tracksOnlyJson = await runCompiled(["task", "list", "--tracks", "--json"], tmpDir);
      expect(expectJson<string[]>(tracksOnlyJson).sort()).toEqual([
        "implement/agent-error-text-investigation",
        "implement/init-template-e2e-tests",
        "implement/template-prompt-fixes",
        "implement/worktree-config-lock-race",
      ].sort());

      const statusJson = await runCompiled(["task", "status", "--json"], tmpDir);
      const projection = expectJson<{
        header: {
          open: number;
          active: number;
          ready: number;
          pending: number;
          blocked: number;
          blockedTracks: number;
        };
        tracks: Array<{ identifier: string; slug?: string; task: { slug?: string } }>;
        tasksById?: Record<string, { id: string }>;
      }>(statusJson);
      expect(projection.header).toEqual({
        open: 12,
        active: 3,
        ready: 7,
        pending: 7,
        blocked: 2,
        blockedTracks: 1,
      });
      const slugs = projection.tracks.map((t) => t.identifier);
      expect(slugs).toContain("implement/worktree-config-lock-race");
      expect(projection.tasksById).toBeUndefined();

      const statusJsonFull = await runCompiled(["task", "status", "--json", "--full"], tmpDir);
      const fullProjection = expectJson<{
        tasksById: Record<string, { id: string }>;
      }>(statusJsonFull);
      expect(fullProjection.tasksById[worktree!.id]?.id).toBe(worktree!.id);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "atomically rejects a plan that mixes valid entries with one that collides on slug",
    async () => {
      const planPath = join(tmpDir, "plan-conflict.json");
      await writeFile(
        planPath,
        JSON.stringify({
          tasks: [
            { name: "a", title: "A", type: "feature", slug: "implement/foo" },
            { name: "b", title: "B", type: "feature", slug: "implement/bar" },
            { name: "c", title: "C", type: "feature", slug: "implement/foo" },
          ],
        }),
        "utf8",
      );

      const dryRun = await runCompiled(["task", "plan", "--file", planPath, "--dry-run"], tmpDir);
      expect(dryRun.exitCode).not.toBe(0);
      expect(dryRun.stderr).toContain("Plan validation failed");

      const afterDryRun = await runCompiled(["task", "list", "--json"], tmpDir);
      expect(expectJson<Array<unknown>>(afterDryRun)).toEqual([]);

      const result = await runCompiled(["task", "plan", "--file", planPath], tmpDir);
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toContain("Plan validation failed");

      const list = await runCompiled(["task", "list", "--json"], tmpDir);
      const tasks = expectJson<Array<unknown>>(list);
      expect(tasks).toEqual([]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "rederives slug swaps atomically",
    async () => {
      const first = await runCompiled(
        ["task", "create", "First", "--slug", "implement/second"],
        tmpDir,
      );
      expect(first.exitCode).toBe(0);
      const second = await runCompiled(
        ["task", "create", "Second", "--slug", "implement/first"],
        tmpDir,
      );
      expect(second.exitCode).toBe(0);

      const apply = await runCompiled(
        ["task", "backfill-slugs", "--rederive", "--apply"],
        tmpDir,
      );
      expect(apply.exitCode).toBe(0);
      expect(apply.stdout).toContain("[ok] Backfilled 2 slug(s)");

      const list = await runCompiled(["task", "list", "--json"], tmpDir);
      const tasks = expectJson<Array<{ title: string; slug?: string }>>(list);
      expect(tasks.find((task) => task.title === "First")?.slug).toBe("implement/first");
      expect(tasks.find((task) => task.title === "Second")?.slug).toBe("implement/second");
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
