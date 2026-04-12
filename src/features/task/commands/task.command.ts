import type { Command } from "commander";
import { getServices } from "@/services.js";
import { output, resolveJsonFlag, warn } from "@/shared/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
import { createTask } from "../usecases/create-task.usecase.js";
import { showTask } from "../usecases/show-task.usecase.js";
import { listTasks } from "../usecases/list-tasks.usecase.js";
import { updateTask } from "../usecases/update-task.usecase.js";
import { closeTask } from "../usecases/close-task.usecase.js";
import { readyTasks } from "../usecases/ready-tasks.usecase.js";
import { captureTaskCandidate } from "../usecases/capture-task-candidate.usecase.js";
import type {
  UpdateTaskInput,
  ListTasksFilters,
  ReadyTasksFilters,
} from "../domain/task-types.js";
import { TASK_TYPES, TASK_STATUSES } from "../domain/task-types.js";
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
  formatTaskBriefingList,
  formatTaskDetail,
  formatTaskList,
  formatTaskSummary,
} from "./task-command-formatters.js";

export function registerTaskCommand(program: Command): void {
  const taskCmd = program
    .command("task")
    .description("Task lifecycle management (br-style issue graph)")
    .option("--json", "Output as JSON");

  registerCreateCommand(taskCmd, program);
  registerQuickCommand(taskCmd, program);
  registerShowCommand(taskCmd, program);
  registerListCommand(taskCmd, program);
  registerUpdateCommand(taskCmd, program);
  registerCloseCommand(taskCmd, program);
  registerReadyCommand(taskCmd, program);
}

// ============================
// task create
// ============================

function registerCreateCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("create <title>")
    .description("Create a new task")
    .option("--description <text>", "Task description")
    .option("--type <type>", `Task type (${TASK_TYPES.join("|")})`)
    .option("--priority <n>", `Priority 0-4 (0=critical, 4=backlog)`)
    .option("--parent <id>", "Parent task id for hierarchy grouping")
    .option("--labels <labels>", "Comma-separated labels")
    .option("--depends-on <ids>", "Comma-separated ids this task blocks on")
    .option("--assignee <name>", "Assignee")
    .option("--silent", "Print only the id (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (title: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const input = buildCreateInput(title, opts);
      const task = await createTask(services.taskStore, input);

      if (opts.silent) {
        console.log(task.id);
        return;
      }

      output(isJson, task, formatTaskSummary);
    });
}

// ============================
// task q (quick capture alias)
// ============================

function registerQuickCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("q <title>")
    .description("Quick capture: create a task and print its id only")
    .option("--type <type>", `Task type (${TASK_TYPES.join("|")})`)
    .option("--priority <n>", "Priority 0-4")
    .option("--labels <labels>", "Comma-separated labels")
    .option("--parent <id>", "Parent task id")
    .option("--json", "Output as JSON")
    .action(async (title: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const input = buildCreateInput(title, opts);
      const task = await createTask(services.taskStore, input);

      if (isJson) {
        output(true, { id: task.id }, () => []);
        return;
      }
      console.log(task.id);
    });
}

// ============================
// task show
// ============================

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

// ============================
// task list
// ============================

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

// ============================
// task update
// ============================

function registerUpdateCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("update <id>")
    .description("Update a task (any field)")
    .option("--title <title>", "New title")
    .option("--description <text>", "New description")
    .option("--status <status>", `New status (${TASK_STATUSES.filter((s) => s !== "closed").join("|")})`)
    .option("--priority <n>", "New priority 0-4")
    .option("--type <type>", `New type (${TASK_TYPES.join("|")})`)
    .option("--parent <id>", "New parent id (empty string clears)")
    .option("--assignee <name>", "New assignee (empty string clears)")
    .option("--add-label <labels>", "Comma-separated labels to add")
    .option("--remove-label <labels>", "Comma-separated labels to remove")
    .option("--claim", "Atomic: set assignee to current session AND status to in_progress")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const patch: UpdateTaskInput = {
        title: opts.title,
        description: opts.description,
        status: parseStatus(opts.status),
        priority: parsePriority(opts.priority),
        type: parseType(opts.type),
        parentId: opts.parent,
        assignee: opts.assignee,
        addLabels: parseList(opts.addLabel),
        removeLabels: parseList(opts.removeLabel),
      };

      let claim: { sessionId: string } | undefined;
      if (opts.claim) {
        const session = await services.sessionDetect.detect(process.cwd());
        if (!session) {
          throw new MaestroError("Could not detect current session for --claim", [
            "Set MAESTRO_AGENT or run from an agent environment",
            "Or assign manually: maestro task update <id> --assignee <name>",
          ]);
        }
        claim = { sessionId: `${session.agent}-${session.sessionId}` };
      }

      if (!hasAnyPatchField(patch) && !claim) {
        throw new MaestroError("No update specified", [
          "Pass at least one field: --title, --description, --status, --priority, --type,",
          "--parent, --assignee, --add-label, --remove-label, or --claim",
        ]);
      }

      const updated = await updateTask(services.taskStore, id, { patch, claim });
      output(isJson, updated, (t) => [
        `[ok] Task updated: ${t.id}`,
        `  Status: ${t.status}`,
        `  Priority: P${t.priority}`,
        ...(t.assignee ? [`  Assignee: ${t.assignee}`] : []),
        ...(t.labels.length > 0 ? [`  Labels: ${t.labels.join(", ")}`] : []),
      ]);
    });
}

// ============================
// task close
// ============================

function registerCloseCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("close <id>")
    .description("Close a task")
    .option("--reason <text>", "Why the task was closed")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const closed = await closeTask(services.taskStore, id, { reason: opts.reason });
      try {
        await captureTaskCandidate(services.taskCandidateStore, closed);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        warn(`Task ${closed.id} closed, but hint capture failed: ${message}`);
      }

      output(isJson, closed, (t) => [
        `[ok] Task closed: ${t.id}`,
        `  Title: ${t.title}`,
        ...(t.closeReason ? [`  Reason: ${t.closeReason}`] : []),
      ]);
    });
}

// ============================
// task ready
// ============================

function registerReadyCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("ready")
    .description("List actionable tasks (unblocked leaves)")
    .option("--limit <n>", "Maximum tasks to return (default 20, 0 = unlimited)")
    .option("--label <label>", "Filter by label")
    .option("--priority <n>", "Filter by priority 0-4")
    .option("--type <type>", `Filter by type (${TASK_TYPES.join("|")})`)
    .option("--assignee <name>", "Filter by assignee")
    .option("--unassigned", "Only unassigned tasks")
    .option("--include-deferred", "Include deferred tasks in the results")
    .option("--no-hints", "Disable lesson hints surfaced from past closes")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      // Commander's --no-X pattern: opts.hints is true by default, false
      // when --no-hints is passed. Active memory is on by default.
      const showHints = opts.hints !== false;

      const filters: ReadyTasksFilters = {
        limit: parseLimit(opts.limit),
        label: opts.label,
        priority: parsePriority(opts.priority),
        type: parseType(opts.type),
        assignee: opts.assignee,
        unassigned: opts.unassigned === true,
        includeDeferred: opts.includeDeferred === true,
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
