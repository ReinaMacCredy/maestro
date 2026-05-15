import { Command } from "commander";
import { buildV2Services } from "../providers/build-services.js";
import { taskFromSpec } from "../service/task-from-spec.usecase.js";
import { taskClaim } from "../service/task-claim.usecase.js";
import { taskBlock } from "../service/task-block.usecase.js";
import { taskAbandon } from "../service/task-abandon.usecase.js";
import { taskVerify } from "../service/task-verify.usecase.js";
import { taskShip } from "../service/task-ship.usecase.js";
import { TaskNotFoundError } from "../repo/task-store.port.js";
import { TaskTransitionError } from "../types/task-state.js";

export interface TaskCommandV2Options {
  readonly resolveRepoRoot: () => string;
}

function findOrCreateTaskCommand(program: Command): Command {
  const existing = program.commands.find((c) => c.name() === "task");
  if (existing) return existing;
  return program.command("task").description("Task lifecycle (v2)");
}

// Remove v1 subcommands that v2 overrides. v1 task claim / block / verify have
// different signatures and semantics; on the harness-os branch v2 owns these
// verbs. v1 versions return in Phase 4 only if a migration test pins them.
function detachV1Overrides(task: Command, overrides: readonly string[]): void {
  for (const name of overrides) {
    const idx = task.commands.findIndex((c) => c.name() === name);
    if (idx !== -1) {
      task.commands.splice(idx, 1);
    }
  }
}

function reportError(verb: string, err: unknown): void {
  if (err instanceof TaskNotFoundError || err instanceof TaskTransitionError) {
    console.error(`maestro ${verb}: ${(err as Error).message}`);
    process.exitCode = 1;
    return;
  }
  throw err;
}

export function registerTaskV2Commands(program: Command, opts: TaskCommandV2Options): void {
  const task = findOrCreateTaskCommand(program);
  detachV1Overrides(task, ["claim", "block", "verify", "ship"]);

  task
    .command("from-spec <path>")
    .description("Create a v2 task in draft from a product-spec markdown file")
    .action(async (pathArg: string): Promise<void> => {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const services = buildV2Services({ repoRoot });
        const created = await taskFromSpec(
          {
            repoRoot,
            specStore: services.specStore,
            taskStore: services.taskStore,
            evidenceStore: services.evidenceStore,
          },
          pathArg,
        );
        console.log(`${created.id} draft (${created.slug})`);
      } catch (err) {
        reportError("task from-spec", err);
      }
    });

  const claimAction = async (id: string, flags: { agent?: string }): Promise<void> => {
    try {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildV2Services({ repoRoot });
      const claimed = await taskClaim(
        {
          taskStore: services.taskStore,
          evidenceStore: services.evidenceStore,
          planStore: services.planStore,
        },
        { id, agentId: flags.agent },
      );
      console.log(`${claimed.id} claimed${flags.agent ? ` by ${flags.agent}` : ""}`);
    } catch (err) {
      reportError("task claim", err);
    }
  };

  task
    .command("claim <id>")
    .description("Claim a task (draft -> claimed)")
    .option("--agent <agent-id>", "agent identifier recorded on the task and evidence row")
    .action(claimAction);

  program
    .command("claim <id>")
    .description("Hot-path alias for `task claim`")
    .option("--agent <agent-id>", "agent identifier recorded on the task and evidence row")
    .action(claimAction);

  const blockAction = async (id: string, flags: { reason?: string }): Promise<void> => {
    if (!flags.reason) {
      console.error("maestro task block: --reason is required");
      process.exitCode = 1;
      return;
    }
    try {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildV2Services({ repoRoot });
      const blocked = await taskBlock(
        {
          taskStore: services.taskStore,
          evidenceStore: services.evidenceStore,
        },
        { id, reason: flags.reason },
      );
      console.log(`${blocked.id} blocked: ${flags.reason}`);
    } catch (err) {
      reportError("task block", err);
    }
  };

  task
    .command("block <id>")
    .description("Block a task with a reason (claimed | doing | verifying -> blocked)")
    .requiredOption("--reason <text>", "human-readable explanation of the blocker")
    .action(blockAction);

  program
    .command("block <id>")
    .description("Hot-path alias for `task block`")
    .requiredOption("--reason <text>", "human-readable explanation of the blocker")
    .action(blockAction);

  const abandonAction = async (id: string, flags: { reason?: string }): Promise<void> => {
    if (!flags.reason) {
      console.error("maestro task abandon: --reason is required");
      process.exitCode = 1;
      return;
    }
    try {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildV2Services({ repoRoot });
      const abandoned = await taskAbandon(
        {
          taskStore: services.taskStore,
          evidenceStore: services.evidenceStore,
          planStore: services.planStore,
        },
        { id, reason: flags.reason },
      );
      console.log(`${abandoned.id} abandoned: ${flags.reason}`);
    } catch (err) {
      reportError("task abandon", err);
    }
  };

  task
    .command("abandon <id>")
    .description("Abandon a task with a reason (any non-terminal -> abandoned)")
    .requiredOption("--reason <text>", "human-readable explanation of abandonment")
    .action(abandonAction);

  program
    .command("abandon <id>")
    .description("Hot-path alias for `task abandon`")
    .requiredOption("--reason <text>", "human-readable explanation of abandonment")
    .action(abandonAction);

  const verifyAction = async function (
    this: Command,
    id: string,
    flags: { json?: boolean },
  ): Promise<void> {
    try {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildV2Services({ repoRoot });
      const result = await taskVerify(
        {
          repoRoot,
          taskStore: services.taskStore,
          evidenceStore: services.evidenceStore,
          architectureRules: services.architectureRules,
        },
        { id },
      );
      const wantJson = flags.json === true || this.optsWithGlobals().json === true;
      if (wantJson) {
        console.log(
          JSON.stringify(
            {
              id: result.task.id,
              state: result.task.state,
              verdict: result.verdict,
              violations: result.violations,
            },
            null,
            2,
          ),
        );
      } else if (result.verdict === "PASS") {
        console.log(`${result.task.id} verified -> ready (PASS)`);
      } else {
        console.log(`${result.task.id} verify FAIL (${result.violations.length} violation(s))`);
        for (const v of result.violations) {
          const loc = v.line ? `${v.file}:${v.line}` : v.file;
          console.log(`  [${v.severity}] ${v.rule_id} ${loc}: ${v.message}`);
        }
      }
      if (result.verdict === "FAIL") process.exitCode = 1;
    } catch (err) {
      reportError("task verify", err);
    }
  };

  task
    .command("verify <id>")
    .description("Run lints; PASS auto-advances verifying -> ready, FAIL stays at verifying")
    .option("--json", "emit JSON result with task, verdict, and violations")
    .action(verifyAction);

  program
    .command("verify <id>")
    .description("Hot-path alias for `task verify`")
    .option("--json", "emit JSON result with task, verdict, and violations")
    .action(verifyAction);

  const shipAction = async (id: string, flags: { prUrl?: string }): Promise<void> => {
    try {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildV2Services({ repoRoot });
      const shipped = await taskShip(
        {
          taskStore: services.taskStore,
          evidenceStore: services.evidenceStore,
          planStore: services.planStore,
        },
        { id, pr_url: flags.prUrl },
      );
      console.log(`${shipped.id} shipped${flags.prUrl ? ` (${flags.prUrl})` : ""}`);
    } catch (err) {
      reportError("task ship", err);
    }
  };

  task
    .command("ship <id>")
    .description("Mark a ready task shipped (ready -> shipped)")
    .option("--pr-url <url>", "PR URL recorded on the task")
    .action(shipAction);

  program
    .command("ship <id>")
    .description("Hot-path alias for `task ship`")
    .option("--pr-url <url>", "PR URL recorded on the task")
    .action(shipAction);
}
