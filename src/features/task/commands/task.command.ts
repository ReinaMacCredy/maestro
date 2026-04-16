import { Command, Option } from "commander";
import { getServices } from "@/services.js";
import { output, resolveJsonFlag, warn } from "@/shared/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
import { createTask } from "../usecases/create-task.usecase.js";
import { showTask } from "../usecases/show-task.usecase.js";
import { listTasks } from "../usecases/list-tasks.usecase.js";
import { updateTask } from "../usecases/update-task.usecase.js";
import { claimTask } from "../usecases/claim-task.usecase.js";
import { unclaimTask } from "../usecases/unclaim-task.usecase.js";
import { blockTasks, unblockTasks } from "../usecases/manage-task-dependencies.usecase.js";
import { closeTask } from "../usecases/close-task.usecase.js";
import { releaseOwnedTasks } from "../usecases/release-owned-tasks.usecase.js";
import { readyTasks } from "../usecases/ready-tasks.usecase.js";
import { captureTaskCandidate } from "../usecases/capture-task-candidate.usecase.js";
import type {
  ListTasksFilters,
  ReadyTasksFilters,
  Task,
  UpdateTaskInput,
} from "../domain/task-types.js";
import { TASK_STATUSES, TASK_TYPES } from "../domain/task-types.js";
import {
  buildCreateInput,
  hasAnyPatchField,
  parseLimit,
  parseList,
  parsePriority,
  parseStatus,
  parseType,
} from "./task-command-parsers.js";
import {
  taskCompletedViaUpdateStatus,
  taskDependencyCommandsRenamed,
  taskUpdateClaimViaDedicatedCommand,
  taskUpdateOwnershipViaClaim,
} from "../domain/task-errors.js";
import {
  formatTaskBriefingList,
  formatTaskDetail,
  formatTaskList,
  formatTaskSummary,
} from "./task-command-formatters.js";

export function registerTaskCommand(program: Command): void {
  const taskCmd = program
    .command("task")
    .description("Task lifecycle management (Claude-style blocker graph)")
    .option("--json", "Output as JSON");

  registerCreateCommand(taskCmd, program);
  registerQuickCommand(taskCmd, program);
  registerShowCommand(taskCmd, program);
  registerListCommand(taskCmd, program);
  registerUpdateCommand(taskCmd, program);
  registerClaimCommand(taskCmd, program);
  registerUnclaimCommand(taskCmd, program);
  registerReleaseOwnedCommand(taskCmd, program);
  registerBlockCommand(taskCmd, program);
  registerUnblockCommand(taskCmd, program);
  registerLegacyDepsCommand(taskCmd);
  registerCloseCommand(taskCmd);
  registerReadyCommand(taskCmd, program);
}

function registerCreateCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("create <title>")
    .description("Create a new task")
    .option("--description <text>", "Task description")
    .option("--type <type>", `Task type (${TASK_TYPES.join("|")})`)
    .option("--priority <n>", "Priority 0-4 (0=critical, 4=backlog)")
    .option("--parent <id>", "Parent task id for hierarchy grouping")
    .option("--labels <labels>", "Comma-separated labels")
    .option("--blocked-by <ids>", "Comma-separated blocker task ids")
    .addOption(new Option("--assignee <name>").hideHelp())
    .option("--silent", "Print only the id (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (title: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      if (opts.assignee !== undefined) {
        throw taskUpdateOwnershipViaClaim();
      }

      const input = buildCreateInput(title, {
        description: opts.description,
        type: opts.type,
        priority: opts.priority,
        parent: opts.parent,
        labels: opts.labels,
        blockedBy: opts.blockedBy,
      });
      const task = await createTask(services.taskStore, input);

      if (opts.silent) {
        console.log(task.id);
        return;
      }

      output(isJson, task, formatTaskSummary);
    });
}

function registerQuickCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("q <title>")
    .description("Quick capture: create a task and print its id only")
    .option("--type <type>", `Task type (${TASK_TYPES.join("|")})`)
    .option("--priority <n>", "Priority 0-4")
    .option("--labels <labels>", "Comma-separated labels")
    .option("--parent <id>", "Parent task id")
    .option("--blocked-by <ids>", "Comma-separated blocker task ids")
    .option("--json", "Output as JSON")
    .action(async (title: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const input = buildCreateInput(title, {
        type: opts.type,
        priority: opts.priority,
        labels: opts.labels,
        parent: opts.parent,
        blockedBy: opts.blockedBy,
      });
      const task = await createTask(services.taskStore, input);

      if (isJson) {
        output(true, { id: task.id }, () => []);
        return;
      }
      console.log(task.id);
    });
}

function registerShowCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("show <id>")
    .description("Show task details")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const task = await showTask(services.taskStore, id);
      output(isJson, task, formatTaskDetail);
    });
}

function registerListCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("list")
    .description("List tasks with optional filters")
    .option("--status <status>", `Filter by status (${TASK_STATUSES.join("|")})`)
    .option("--priority <n>", "Filter by priority 0-4")
    .option("--type <type>", `Filter by type (${TASK_TYPES.join("|")})`)
    .option("--label <label>", "Filter by label (single)")
    .option("--parent <id>", "Filter by parent task id")
    .option("--assignee <name>", "Filter by assignee")
    .option("--limit <n>", "Maximum number of tasks to return")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const filters: ListTasksFilters = {
        status: parseStatus(opts.status),
        priority: parsePriority(opts.priority),
        type: parseType(opts.type),
        label: opts.label,
        parentId: opts.parent,
        assignee: opts.assignee,
        limit: parseLimit(opts.limit),
      };

      const tasks = await listTasks(services.taskStore, filters);
      output(isJson, tasks, formatTaskList);
    });
}

function registerUpdateCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("update <id>")
    .description("Update task fields or move task status explicitly")
    .option("--title <title>", "New title")
    .option("--description <text>", "New description")
    .option("--status <status>", `New status (${TASK_STATUSES.join("|")})`)
    .option("--reason <text>", "Completion reason when --status completed")
    .option("--priority <n>", "New priority 0-4")
    .option("--type <type>", `New type (${TASK_TYPES.join("|")})`)
    .option("--parent <id>", "New parent id (empty string clears)")
    .addOption(new Option("--assignee <name>").hideHelp())
    .option("--add-label <labels>", "Comma-separated labels to add")
    .option("--remove-label <labels>", "Comma-separated labels to remove")
    .addOption(new Option("--claim").hideHelp())
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      if (opts.assignee !== undefined) {
        throw taskUpdateOwnershipViaClaim();
      }
      if (opts.claim === true) {
        throw taskUpdateClaimViaDedicatedCommand();
      }

      const patch: UpdateTaskInput = {
        title: opts.title,
        description: opts.description,
        status: parseStatus(opts.status),
        reason: opts.reason,
        priority: parsePriority(opts.priority),
        type: parseType(opts.type),
        parentId: opts.parent,
        addLabels: parseList(opts.addLabel),
        removeLabels: parseList(opts.removeLabel),
      };

      if (!hasAnyPatchField(patch)) {
        throw new MaestroError("No update specified", [
          "Pass at least one field such as --title, --description, --status, --reason,",
          "--priority, --type, --parent, --add-label, or --remove-label",
        ]);
      }

      const updated = await updateTask(services.taskStore, id, patch);
      await maybeCaptureCompletionHint(updated);

      output(isJson, updated, (task) => [
        `[ok] Task updated: ${task.id}`,
        `  Status: ${task.status}`,
        `  Priority: P${task.priority}`,
        ...(task.assignee ? [`  Assignee: ${task.assignee}`] : []),
        ...(task.blockedBy.length > 0 ? [`  Blocked by: ${task.blockedBy.join(", ")}`] : []),
        ...(task.blocks.length > 0 ? [`  Blocks: ${task.blocks.join(", ")}`] : []),
        ...(task.closeReason ? [`  Reason: ${task.closeReason}`] : []),
      ]);
    });
}

function registerClaimCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("claim <id>")
    .description("Claim exclusive ownership of a task")
    .option("--force", "Take over a task already claimed by another session")
    .option("--busy-check", "Reject the claim if this session already owns unresolved work")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const sessionId = await resolveOwnershipSessionId(opts.session);

      const claimed = await claimTask(services.taskStore, id, {
        sessionId,
        force: opts.force === true,
        checkBusy: opts.busyCheck === true,
      });

      output(isJson, claimed, (task) => [
        `[ok] Task claimed: ${task.id}`,
        `  Assignee: ${task.assignee}`,
        `  Status: ${task.status}`,
      ]);
    });
}

function registerUnclaimCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("unclaim <id>")
    .description("Release task ownership")
    .option("--force", "Release a task owned by another session")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const sessionId = await resolveOwnershipSessionId(opts.session);

      const unclaimed = await unclaimTask(services.taskStore, id, {
        sessionId,
        force: opts.force === true,
      });

      output(isJson, unclaimed, (task) => [
        `[ok] Task unclaimed: ${task.id}`,
        `  Status: ${task.status}`,
      ]);
    });
}

function registerReleaseOwnedCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("release-owned <sessionId>")
    .description("Release unresolved tasks owned by a dead or stale session")
    .option("--json", "Output as JSON")
    .action(async (sessionId: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const released = await releaseOwnedTasks(services.taskStore, sessionId.trim());

      output(isJson, released, (tasks) => {
        if (tasks.length === 0) {
          return [`No unresolved tasks owned by ${sessionId.trim()}`];
        }
        return [
          `[ok] Released ${tasks.length} task(s) owned by ${sessionId.trim()}`,
          ...tasks.map((task) => `  ${task.id} -> ${task.status}`),
        ];
      });
    });
}

function registerBlockCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("block <id> <blockedTaskIds...>")
    .description("Mark this task as blocking the target task ids")
    .option("--json", "Output as JSON")
    .action(async (id: string, blockedTaskIds: string[], opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const updated = await blockTasks(services.taskStore, id, blockedTaskIds);

      output(isJson, updated, (task) => [
        `[ok] Blockers added: ${task.id}`,
        ...(task.blocks.length > 0 ? [`  Blocks: ${task.blocks.join(", ")}`] : ["  Blocks: none"]),
      ]);
    });
}

function registerUnblockCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("unblock <id> <blockedTaskIds...>")
    .description("Remove blocker edges from this task to the target task ids")
    .option("--json", "Output as JSON")
    .action(async (id: string, blockedTaskIds: string[], opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const updated = await unblockTasks(services.taskStore, id, blockedTaskIds);

      output(isJson, updated, (task) => [
        `[ok] Blockers removed: ${task.id}`,
        ...(task.blocks.length > 0 ? [`  Blocks: ${task.blocks.join(", ")}`] : ["  Blocks: none"]),
      ]);
    });
}

function registerLegacyDepsCommand(taskCmd: Command): void {
  const depsCmd = taskCmd
    .command("deps")
    .description("Legacy compatibility shim for renamed blocker commands");

  depsCmd
    .command("add <id> <dependencyIds...>")
    .description("Legacy compatibility shim")
    .action(() => {
      throw taskDependencyCommandsRenamed();
    });

  depsCmd
    .command("remove <id> <dependencyIds...>")
    .description("Legacy compatibility shim")
    .action(() => {
      throw taskDependencyCommandsRenamed();
    });
}

function registerCloseCommand(taskCmd: Command): void {
  taskCmd
    .command("close <id>")
    .description("Legacy compatibility shim; completion moved to task update")
    .action(() => {
      throw taskCompletedViaUpdateStatus();
    });
}

async function resolveOwnershipSessionId(explicitSessionId: string | undefined): Promise<string> {
  if (explicitSessionId !== undefined) {
    const trimmed = explicitSessionId.trim();
    if (trimmed.length === 0) {
      throw new MaestroError("Invalid --session value", [
        "Pass a non-empty session id such as 'codex-1234' or 'operator-recovery'",
      ]);
    }
    return trimmed;
  }

  const services = getServices();
  const session = await services.sessionDetect.detect(process.cwd());
  if (!session) {
    throw new MaestroError("Could not detect current session for task ownership", [
      "Set CODEX_THREAD_ID or run from an agent environment",
      "Or pass --session <id> for an explicit operator or CI override",
    ]);
  }
  return `${session.agent}-${session.sessionId}`;
}

function registerReadyCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("ready")
    .description("List actionable pending tasks with no unresolved blockers")
    .option("--limit <n>", "Maximum tasks to return (default 20, 0 = unlimited)")
    .option("--label <label>", "Filter by label")
    .option("--priority <n>", "Filter by priority 0-4")
    .option("--type <type>", `Filter by type (${TASK_TYPES.join("|")})`)
    .option("--assignee <name>", "Filter by assignee")
    .option("--unassigned", "Only include unassigned tasks")
    .option("--no-hints", "Disable lesson hints surfaced from past completed tasks")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const showHints = opts.hints !== false;

      const filters: ReadyTasksFilters = {
        limit: parseLimit(opts.limit),
        label: opts.label,
        priority: parsePriority(opts.priority),
        type: parseType(opts.type),
        assignee: opts.assignee,
        unassigned: opts.unassigned === true,
      };

      const briefings = await readyTasks(
        services.taskStore,
        filters,
        new Date(),
        showHints ? services.taskCandidateStore : undefined,
      );
      output(isJson, briefings, formatTaskBriefingList);
    });
}

async function maybeCaptureCompletionHint(task: Task): Promise<void> {
  if (task.status !== "completed") {
    return;
  }

  const services = getServices();
  try {
    await captureTaskCandidate(services.taskCandidateStore, task);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    warn(`Task ${task.id} completed, but hint capture failed: ${message}`);
  }
}
