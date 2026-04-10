/**
 * Task command handler.
 * Registers the `task` parent command and all subcommands.
 *
 * Round one covers: create, q, show, list, update, close, ready.
 * Subcommands are layered in via subsequent commits; this file grows
 * monotonically until all seven are wired.
 */
import type { Command } from "commander";
import { getServices } from "@/services.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
import { createTask } from "../usecases/create-task.usecase.js";
import { showTask } from "../usecases/show-task.usecase.js";
import { listTasks } from "../usecases/list-tasks.usecase.js";
import { updateTask } from "../usecases/update-task.usecase.js";
import type {
  Task,
  TaskPriority,
  TaskType,
  TaskStatus,
  CreateTaskInput,
  UpdateTaskInput,
  ListTasksFilters,
} from "../domain/task-types.js";
import { TASK_PRIORITIES, TASK_TYPES, TASK_STATUSES } from "../domain/task-types.js";

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
// Helpers
// ============================

interface CreateOpts {
  description?: string;
  type?: string;
  priority?: string;
  parent?: string;
  labels?: string;
  dependsOn?: string;
  assignee?: string;
}

function buildCreateInput(title: string, opts: CreateOpts): CreateTaskInput {
  return {
    title,
    description: opts.description,
    type: parseType(opts.type),
    priority: parsePriority(opts.priority),
    parentId: opts.parent,
    labels: parseList(opts.labels),
    dependsOn: parseList(opts.dependsOn),
    assignee: opts.assignee,
  };
}

function parseType(value: string | undefined): TaskType | undefined {
  if (value === undefined) return undefined;
  if ((TASK_TYPES as readonly string[]).includes(value)) {
    return value as TaskType;
  }
  throw new MaestroError(`Invalid --type '${value}'`, [
    `Valid types: ${TASK_TYPES.join(", ")}`,
  ]);
}

function parseStatus(value: string | undefined): TaskStatus | undefined {
  if (value === undefined) return undefined;
  if ((TASK_STATUSES as readonly string[]).includes(value)) {
    return value as TaskStatus;
  }
  throw new MaestroError(`Invalid --status '${value}'`, [
    `Valid statuses: ${TASK_STATUSES.join(", ")}`,
  ]);
}

function parseLimit(value: string | undefined): number | undefined {
  if (value === undefined) return undefined;
  const n = Number.parseInt(value, 10);
  if (Number.isNaN(n) || n < 0) {
    throw new MaestroError(`Invalid --limit '${value}'`, [
      "Limit must be a non-negative integer (0 = unlimited)",
    ]);
  }
  return n;
}

function hasAnyPatchField(patch: UpdateTaskInput): boolean {
  return (
    patch.title !== undefined ||
    patch.description !== undefined ||
    patch.status !== undefined ||
    patch.priority !== undefined ||
    patch.type !== undefined ||
    patch.parentId !== undefined ||
    patch.assignee !== undefined ||
    (patch.addLabels !== undefined && patch.addLabels.length > 0) ||
    (patch.removeLabels !== undefined && patch.removeLabels.length > 0) ||
    patch.deferUntil !== undefined
  );
}

function parsePriority(value: string | undefined): TaskPriority | undefined {
  if (value === undefined) return undefined;
  const n = Number.parseInt(value, 10);
  if (Number.isNaN(n) || !(TASK_PRIORITIES as readonly number[]).includes(n)) {
    throw new MaestroError(`Invalid --priority '${value}'`, [
      "Priority must be one of 0, 1, 2, 3, 4",
      "0 = critical, 4 = backlog",
    ]);
  }
  return n as TaskPriority;
}

function parseList(value: string | undefined): readonly string[] | undefined {
  if (value === undefined) return undefined;
  return value
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

// ============================
// Formatters
// ============================

function formatTaskSummary(task: Task): string[] {
  return [
    `[ok] Task created: ${task.id}`,
    `  Title: ${task.title}`,
    `  Status: ${task.status}`,
    `  Priority: P${task.priority}`,
    `  Type: ${task.type}`,
    ...(task.parentId ? [`  Parent: ${task.parentId}`] : []),
    ...(task.labels.length > 0 ? [`  Labels: ${task.labels.join(", ")}`] : []),
    ...(task.dependsOn.length > 0 ? [`  Depends on: ${task.dependsOn.join(", ")}`] : []),
  ];
}

function formatTaskList(tasks: readonly Task[]): string[] {
  if (tasks.length === 0) {
    return ["No tasks found"];
  }

  const lines: string[] = [`${tasks.length} task(s)`, ""];
  for (const t of tasks) {
    const status = t.status.padEnd(12);
    const prio = `P${t.priority}`;
    const title = t.title.length > 40 ? `${t.title.slice(0, 37)}...` : t.title;
    lines.push(`${t.id}  ${prio}  ${status}  ${title}`);
  }
  return lines;
}

function formatTaskDetail(task: Task): string[] {
  const lines: string[] = [
    `Task: ${task.id}`,
    `  Title: ${task.title}`,
    `  Status: ${task.status}`,
    `  Priority: P${task.priority}`,
    `  Type: ${task.type}`,
    `  Created: ${task.createdAt}`,
    `  Updated: ${task.updatedAt}`,
  ];

  if (task.description) lines.push(`  Description: ${task.description}`);
  if (task.parentId) lines.push(`  Parent: ${task.parentId}`);
  if (task.assignee) lines.push(`  Assignee: ${task.assignee}`);
  if (task.labels.length > 0) lines.push(`  Labels: ${task.labels.join(", ")}`);
  if (task.dependsOn.length > 0) lines.push(`  Depends on: ${task.dependsOn.join(", ")}`);
  if (task.deferUntil) lines.push(`  Deferred until: ${task.deferUntil}`);
  if (task.closeReason) lines.push(`  Close reason: ${task.closeReason}`);

  return lines;
}
