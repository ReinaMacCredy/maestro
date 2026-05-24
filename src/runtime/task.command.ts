import { Command } from "commander";
import { registerTaskObserveCommand } from "../features/runtime/commands/task-observe.command.js";
import { buildCoreServices } from "../providers/build-services.js";
import {
  FsContractStoreAdapter,
  FsContractVersionStoreAdapter,
} from "@/shared/domain/task/index.js";
import { FsVerdictStoreAdapter } from "@/features/verdict/adapters/fs-verdict-store.adapter.js";
import { parseNonNegativeInt, parsePositiveInt } from "../shared/lib/cli-options.js";
import { stringifyForOutput } from "../shared/lib/output.js";
import { taskFromSpec, SpecFileNotFoundError } from "../service/task-from-spec.usecase.js";
import { taskClaim } from "../service/task-claim.usecase.js";
import { taskBlock } from "../service/task-block.usecase.js";
import { taskAbandon } from "../service/task-abandon.usecase.js";
import { taskVerify, TaskVerifyReasonRequiredError } from "../service/task-verify.usecase.js";
import { taskShip } from "../service/task-ship.usecase.js";
import {
  taskSplit,
  TaskSplitInvalidStateError,
  TaskSplitNotClaimantError,
  EmptyChildInputsError,
} from "../service/task-split.usecase.js";
import { MissionTerminalGuardError } from "../service/assert-mission-active.js";
import { refreshNowMdFromServices } from "../service/refresh-now-md.js";
import { TaskNotFoundError } from "../repo/task-store.port.js";
import { TaskTransitionError, TASK_STATES, type TaskState } from "../types/task-state.js";
import type { Task, TaskId } from "../types/task.js";
import { summarizeTask } from "../shared/lib/projection.js";

export interface TaskCommandOptions {
  readonly resolveRepoRoot: () => string;
}

function findOrCreateTaskCommand(program: Command): Command {
  const existing = program.commands.find((c) => c.name() === "task");
  if (existing) return existing;
  return program.command("task").description("Task lifecycle");
}

function reportError(verb: string, err: unknown): void {
  if (
    err instanceof TaskNotFoundError ||
    err instanceof TaskTransitionError ||
    err instanceof TaskVerifyReasonRequiredError ||
    err instanceof MissionTerminalGuardError ||
    err instanceof TaskSplitInvalidStateError ||
    err instanceof TaskSplitNotClaimantError ||
    err instanceof EmptyChildInputsError
  ) {
    console.error(`maestro ${verb}: ${(err as Error).message}`);
    process.exitCode = 1;
    return;
  }
  if (err instanceof SpecFileNotFoundError) {
    console.error(`maestro ${verb}: ${err.message}`);
    const looksLikeSlug = !err.inputArg.includes("/") && !err.inputArg.endsWith(".md");
    if (looksLikeSlug) {
      console.error(`  Did you mean: maestro ${verb} .maestro/specs/${err.inputArg}.md ?`);
    }
    console.error(`  List specs:  ls .maestro/specs/`);
    process.exitCode = 1;
    return;
  }
  throw err;
}

export function registerTaskCommands(program: Command, opts: TaskCommandOptions): void {
  const task = findOrCreateTaskCommand(program);

  task
    .command("from-spec <path>")
    .description("Create a task in draft from a product-spec markdown file")
    .action(async (pathArg: string): Promise<void> => {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const services = buildCoreServices({ repoRoot });
        const created = await taskFromSpec(
          {
            repoRoot,
            specStore: services.specStore,
            taskStore: services.taskStore,
            evidenceStore: services.evidenceStore,
            observabilityStore: services.observabilityStore,
          },
          pathArg,
        );
        console.log(`${created.id} draft (${created.slug})`);
        await refreshNowMdFromServices(services);
      } catch (err) {
        reportError("task from-spec", err);
      }
    });

  const claimAction = async (
    id: string,
    flags: { agent?: string; skipWorktree?: boolean; tool?: string },
  ): Promise<void> => {
    try {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildCoreServices({ repoRoot });
      const contractStore = new FsContractStoreAdapter(repoRoot);
      const contractVersionStore = new FsContractVersionStoreAdapter(repoRoot);
      const claimed = await taskClaim(
        {
          taskStore: services.taskStore,
          evidenceStore: services.evidenceStore,
          missionStore: services.missionStore,
          observabilityStore: services.observabilityStore,
          worktreeStore: services.worktreeStore,
          handoffEmitter: services.handoffEmitter,
          contractStore,
          contractVersionStore,
          repoRoot,
        },
        {
          id,
          agentId: flags.agent,
          skipWorktree: flags.skipWorktree === true,
          // Default to "cli" so CLI-emitted handoffs carry a routable
          // to_agent. Operators can override with --tool to claim on behalf
          // of a specific agent (e.g. --tool codex).
          tool: flags.tool ?? "cli",
        },
      );
      const worktreeNote = claimed.worktree_path ? ` (worktree ${claimed.worktree_path})` : "";
      console.log(
        `${claimed.id} claimed${flags.agent ? ` by ${flags.agent}` : ""}${worktreeNote}`,
      );
      await refreshNowMdFromServices(services);
    } catch (err) {
      reportError("task claim", err);
    }
  };

  task
    .command("claim <id>")
    .description("Claim a task (draft -> claimed); auto-creates a worktree for heavy-mode specs")
    .option("--agent <agent-id>", "agent identifier recorded on the task and evidence row")
    .option("--skip-worktree", "skip auto-worktree creation even for heavy-mode specs")
    .option(
      "--tool <name>",
      "caller tool name written as to_agent on the auto-emitted handoff (default: cli)",
    )
    .action(claimAction);

  program
    .command("claim <id>")
    .description("Hot-path alias for `task claim`")
    .option("--agent <agent-id>", "agent identifier recorded on the task and evidence row")
    .option("--skip-worktree", "skip auto-worktree creation even for heavy-mode specs")
    .option(
      "--tool <name>",
      "caller tool name written as to_agent on the auto-emitted handoff (default: cli)",
    )
    .action(claimAction);

  const splitAction = async (
    parentId: string,
    titles: string[],
    flags: { parallel?: boolean; agent?: string },
  ): Promise<void> => {
    try {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildCoreServices({ repoRoot });
      const children = await taskSplit(
        {
          taskStore: services.taskStore,
          evidenceStore: services.evidenceStore,
          missionStore: services.missionStore,
        },
        {
          id: parentId as TaskId,
          titles,
          ...(flags.parallel === true ? { parallel: true } : {}),
          ...(flags.agent !== undefined ? { agentId: flags.agent } : {}),
        },
      );
      for (const c of children) {
        console.log(`${c.id} draft (${c.slug})`);
      }
      await refreshNowMdFromServices(services);
    } catch (err) {
      reportError("task split", err);
    }
  };

  task
    .command("split <parent-id> <titles...>")
    .description(
      "Split a claimed/doing parent into child tasks (each child draft, parent gains blocked_by refs)",
    )
    .option("--parallel", "create children with empty blocked_by (no sequential chain)")
    .option("--agent <agent-id>", "asserts the parent is assigned to this agent before splitting")
    .action(splitAction);

  const blockAction = async (
    id: string,
    flags: { reason?: string; tool?: string },
  ): Promise<void> => {
    if (!flags.reason) {
      console.error("maestro task block: --reason is required");
      process.exitCode = 1;
      return;
    }
    try {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildCoreServices({ repoRoot });
      const blocked = await taskBlock(
        {
          taskStore: services.taskStore,
          evidenceStore: services.evidenceStore,
          missionStore: services.missionStore,
          observabilityStore: services.observabilityStore,
          handoffEmitter: services.handoffEmitter,
        },
        // Default to "cli" so block handoffs carry a routable to_agent;
        // operators can override with --tool when blocking on behalf of
        // another agent.
        { id, reason: flags.reason, tool: flags.tool ?? "cli" },
      );
      console.log(`${blocked.id} blocked: ${flags.reason}`);
      await refreshNowMdFromServices(services);
    } catch (err) {
      reportError("task block", err);
    }
  };

  task
    .command("block <id>")
    .description("Block a task with a reason (claimed | doing | verifying -> blocked)")
    .requiredOption("--reason <text>", "human-readable explanation of the blocker")
    .option(
      "--tool <name>",
      "caller tool name written as to_agent on the auto-emitted handoff (default: cli)",
    )
    .action(blockAction);

  program
    .command("block <id>")
    .description("Hot-path alias for `task block`")
    .requiredOption("--reason <text>", "human-readable explanation of the blocker")
    .option(
      "--tool <name>",
      "caller tool name written as to_agent on the auto-emitted handoff (default: cli)",
    )
    .action(blockAction);

  const abandonAction = async (id: string, flags: { reason?: string }): Promise<void> => {
    if (!flags.reason) {
      console.error("maestro task abandon: --reason is required");
      process.exitCode = 1;
      return;
    }
    try {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildCoreServices({ repoRoot });
      const abandoned = await taskAbandon(
        {
          taskStore: services.taskStore,
          evidenceStore: services.evidenceStore,
          missionStore: services.missionStore,
          observabilityStore: services.observabilityStore,
        },
        { id, reason: flags.reason },
      );
      console.log(`${abandoned.id} abandoned: ${flags.reason}`);
      await refreshNowMdFromServices(services);
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
    flags: { json?: boolean; verdict?: string; reason?: string },
  ): Promise<void> {
    try {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildCoreServices({ repoRoot });
      let explicit: "HUMAN" | "BLOCK" | undefined;
      if (flags.verdict !== undefined) {
        const v = flags.verdict.toLowerCase();
        if (v !== "human" && v !== "block") {
          console.error(
            `maestro task verify: --verdict must be 'human' or 'block' (got '${flags.verdict}')`,
          );
          process.exitCode = 1;
          return;
        }
        explicit = v === "human" ? "HUMAN" : "BLOCK";
      }
      const result = await taskVerify(
        {
          repoRoot,
          taskStore: services.taskStore,
          evidenceStore: services.evidenceStore,
          architectureRules: services.architectureRules,
          observabilityStore: services.observabilityStore,
        },
        { id, verdict: explicit, reason: flags.reason },
      );
      const wantJson = flags.json === true || this.optsWithGlobals().json === true;
      if (wantJson) {
        console.log(
          stringifyForOutput({
            id: result.task.id,
            state: result.task.state,
            verdict: result.verdict,
            violations: result.violations,
          }),
        );
      } else if (result.verdict === "PASS") {
        console.log(`${result.task.id} verified -> ready (PASS)`);
      } else if (result.verdict === "HUMAN") {
        console.log(`${result.task.id} verify HUMAN: ${flags.reason}`);
      } else if (result.verdict === "BLOCK") {
        console.log(`${result.task.id} verify BLOCK -> blocked: ${flags.reason}`);
      } else {
        console.log(`${result.task.id} verify FAIL (${result.violations.length} violation(s))`);
        for (const v of result.violations) {
          const loc = v.line ? `${v.file}:${v.line}` : v.file;
          console.log(`  [${v.severity}] ${v.rule_id} ${loc}: ${v.message}`);
        }
      }
      if (result.verdict === "FAIL") process.exitCode = 1;
      else if (result.verdict === "HUMAN") process.exitCode = 2;
      else if (result.verdict === "BLOCK") process.exitCode = 3;
      await refreshNowMdFromServices(services);
    } catch (err) {
      reportError("task verify", err);
    }
  };

  task
    .command("verify <id>")
    .description(
      "Run lints; PASS auto-advances verifying -> ready, FAIL stays at verifying. " +
        "Use --verdict {human,block} --reason to record an explicit human verdict instead.",
    )
    .option("--json", "emit JSON result with task, verdict, and violations")
    .option("--verdict <verdict>", "explicit verdict: 'human' (stay) or 'block' (-> blocked)")
    .option("--reason <text>", "required when --verdict is set")
    .action(verifyAction);

  program
    .command("verify <id>")
    .description("Hot-path alias for `task verify`")
    .option("--json", "emit JSON result with task, verdict, and violations")
    .option("--verdict <verdict>", "explicit verdict: 'human' (stay) or 'block' (-> blocked)")
    .option("--reason <text>", "required when --verdict is set")
    .action(verifyAction);

  const shipAction = async (id: string, flags: { prUrl?: string }): Promise<void> => {
    try {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildCoreServices({ repoRoot });
      const shipped = await taskShip(
        {
          taskStore: services.taskStore,
          evidenceStore: services.evidenceStore,
          missionStore: services.missionStore,
          observabilityStore: services.observabilityStore,
          verdictStore: new FsVerdictStoreAdapter(repoRoot),
        },
        { id, pr_url: flags.prUrl },
      );
      console.log(`${shipped.id} shipped${flags.prUrl ? ` (${flags.prUrl})` : ""}`);
      await refreshNowMdFromServices(services);
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

  const parseLimit = (v: string): number => Math.min(parsePositiveInt(v), 100);

  const listAction = async function (
    this: Command,
    flags: {
      missionId?: string;
      state?: string;
      limit?: number;
      offset?: number;
      json?: boolean;
      full?: boolean;
      all?: boolean;
    },
  ): Promise<void> {
    try {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildCoreServices({ repoRoot });
      let tasks: readonly Task[];
      if (flags.missionId !== undefined) {
        tasks = await services.taskStore.listByMissionId(flags.missionId);
      } else if (flags.state !== undefined) {
        if (!(TASK_STATES as readonly string[]).includes(flags.state)) {
          console.error(
            `maestro task list: --state must be one of ${TASK_STATES.join("|")} (got '${flags.state}')`,
          );
          process.exitCode = 1;
          return;
        }
        tasks = await services.taskStore.listByState(flags.state as TaskState);
      } else {
        tasks = await services.taskStore.list();
      }
      const offset = flags.offset ?? 0;
      // Clamp limit to non-negative even when --all is paired with an offset
      // larger than tasks.length. Otherwise the emitted `limit` field would be
      // negative, breaking downstream pagination math.
      const rawLimit = flags.all === true ? tasks.length - offset : (flags.limit ?? 20);
      const limit = Math.max(rawLimit, 0);
      const page = tasks.slice(offset, offset + limit);
      const wantJson = flags.json === true || this.optsWithGlobals().json === true;
      if (wantJson) {
        const items = flags.full === true
          ? page
          : page.map(summarizeTask);
        console.log(
          stringifyForOutput({ items, total: tasks.length, limit, offset }),
        );
        return;
      }
      if (page.length === 0) {
        console.log("No tasks");
        return;
      }
      for (const t of page) {
        const missionNote = t.mission_id ? ` mission=${t.mission_id}` : "";
        console.log(`${t.id}\t${t.state}\t${t.slug}\t${t.title}${missionNote}`);
      }
    } catch (err) {
      reportError("task list", err);
    }
  };

  task
    .command("list")
    .description("List tasks (filter by --mission-id or --state; paginated)")
    .option("--mission-id <id>", "filter by mission id")
    .option("--state <state>", `filter by state (${TASK_STATES.join("|")})`)
    .option("--limit <n>", "page size (default 20, max 100)", parseLimit)
    .option("--offset <n>", "page offset (default 0)", parseNonNegativeInt)
    .option("--full", "emit full task records in JSON (default: summary projection)")
    .option("--all", "drop the default 20-item cap")
    .option("--json", "emit JSON {items,total,limit,offset}")
    .action(listAction);

  const getAction = async function (
    this: Command,
    id: string,
    flags: { json?: boolean },
  ): Promise<void> {
    try {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildCoreServices({ repoRoot });
      const t = await services.taskStore.get(id);
      if (!t) {
        console.error(`maestro task get: task ${id} not found`);
        process.exitCode = 1;
        return;
      }
      const wantJson = flags.json === true || this.optsWithGlobals().json === true;
      if (wantJson) {
        console.log(stringifyForOutput({ task: t }));
        return;
      }
      console.log(`${t.id} ${t.state} ${t.slug}`);
      console.log(`  title:      ${t.title}`);
      if (t.mission_id) console.log(`  mission_id: ${t.mission_id}`);
      if (t.spec_path) console.log(`  spec_path:  ${t.spec_path}`);
      if (t.assignee) console.log(`  assignee:   ${t.assignee}`);
      if (t.claimed_at) console.log(`  claimed_at: ${t.claimed_at}`);
      if (t.pr_url) console.log(`  pr_url:     ${t.pr_url}`);
      if (t.block_reason) console.log(`  blocked:    ${t.block_reason}`);
      if (t.abandon_reason) console.log(`  abandoned:  ${t.abandon_reason}`);
      console.log(`  created_at: ${t.created_at}`);
      console.log(`  updated_at: ${t.updated_at}`);
    } catch (err) {
      reportError("task get", err);
    }
  };

  task
    .command("get <id>")
    .description("Show a single task by id")
    .option("--json", "emit JSON {task: ...}")
    .action(getAction);

  registerTaskObserveCommand(task, { resolveRepoRoot: opts.resolveRepoRoot });
}
