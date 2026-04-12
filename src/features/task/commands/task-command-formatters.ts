import { formatRelativeAge } from "@/shared/version-format.js";
import type { Task } from "../domain/task-types.js";
import type { TaskHint } from "../usecases/match-candidates.usecase.js";
import type { TaskBriefing } from "../usecases/ready-tasks.usecase.js";

export function formatTaskSummary(task: Task): string[] {
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

export function formatTaskList(tasks: readonly Task[]): string[] {
  if (tasks.length === 0) {
    return ["No tasks found"];
  }

  const lines: string[] = [`${tasks.length} task(s)`, ""];
  for (const task of tasks) {
    const status = task.status.padEnd(12);
    const priority = `P${task.priority}`;
    const title = task.title.length > 40 ? `${task.title.slice(0, 37)}...` : task.title;
    lines.push(`${task.id}  ${priority}  ${status}  ${title}`);
  }
  return lines;
}

export function formatTaskBriefingList(briefings: readonly TaskBriefing[]): string[] {
  if (briefings.length === 0) {
    return ["No tasks found"];
  }

  const lines: string[] = [`${briefings.length} task(s)`, ""];
  for (const briefing of briefings) {
    const status = briefing.status.padEnd(12);
    const priority = `P${briefing.priority}`;
    const title = briefing.title.length > 40 ? `${briefing.title.slice(0, 37)}...` : briefing.title;
    lines.push(`${briefing.id}  ${priority}  ${status}  ${title}`);
    for (const hint of briefing.hints) {
      lines.push(`  >> ${formatHintLine(hint)}`);
    }
    if (briefing.hints.length > 0) {
      lines.push("");
    }
  }
  return lines;
}

function formatHintLine(hint: TaskHint): string {
  const age = formatRelativeAge(hint.capturedAt);
  return `${age} closed ${hint.sourceTaskId}: ${hint.reason}`;
}

export function formatTaskDetail(task: Task): string[] {
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
