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

describe("compiled task status + plan slug conversion", () => {
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

      // Move the worktree task to in_progress so it shows the `o` glyph and
      // the active counter increments. Without this nothing is in_progress.
      const planJson = await runCompiled(["task", "plan", "--file", planPath, "--json"], tmpDir);
      void planJson;

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
      expect(status.stdout).toContain("tasks: 3 active, 7 pending, 2 blocked");
      expect(status.stdout).toContain("implement/worktree-config-lock-race");
      expect(status.stdout).toContain("blocked by implement/template-prompt-fixes");

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

      const statusJson = await runCompiled(["task", "status", "--json"], tmpDir);
      const projection = expectJson<{
        header: { active: number; pending: number; blocked: number };
        tracks: Array<{ identifier: string; slug?: string; task: { slug?: string } }>;
      }>(statusJson);
      expect(projection.header).toEqual({ active: 3, pending: 7, blocked: 2 });
      const slugs = projection.tracks.map((t) => t.identifier);
      expect(slugs).toContain("implement/worktree-config-lock-race");
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

      const result = await runCompiled(["task", "plan", "--file", planPath], tmpDir);
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toContain("Plan validation failed");

      const list = await runCompiled(["task", "list", "--json"], tmpDir);
      const tasks = expectJson<Array<unknown>>(list);
      expect(tasks).toEqual([]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
