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
import type {
  Task,
  TaskPriority,
  TaskType,
  CreateTaskInput,
} from "../domain/task-types.js";
import { TASK_PRIORITIES, TASK_TYPES } from "../domain/task-types.js";

export function registerTaskCommand(program: Command): void {
  const taskCmd = program
    .command("task")
    .description("Task lifecycle management (br-style issue graph)")
    .option("--json", "Output as JSON");

  registerCreateCommand(taskCmd, program);
  registerQuickCommand(taskCmd, program);
  registerShowCommand(taskCmd, program);
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
