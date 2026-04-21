import { formatRelativeAge } from "@/shared/version-format.js";
import type { Task } from "../domain/task-types.js";
import type { TaskShowView } from "../usecases/task-continuation.usecase.js";
import type { TaskHint } from "../usecases/match-candidates.usecase.js";
import type { ReadyTaskPage, TaskBriefing } from "../usecases/ready-tasks.usecase.js";

export type CompactReadyTaskItem = Pick<
  Task,
  "id" | "title" | "status" | "priority" | "type" | "labels" | "parentId" | "assignee"
>;

export interface CompactReadyTaskPayload {
  readonly schemaVersion: 1;
  readonly totalReady: number;
  readonly returned: number;
  readonly hasMore: boolean;
  readonly items: readonly CompactReadyTaskItem[];
}

export function formatTaskSummary(task: Task): string[] {
  return [
    `[ok] Task created: ${task.id}`,
    `  Title: ${task.title}`,
    `  Status: ${task.status}`,
    `  Priority: P${task.priority}`,
    `  Type: ${task.type}`,
    ...(task.parentId ? [`  Parent: ${task.parentId}`] : []),
    ...(task.labels.length > 0 ? [`  Labels: ${task.labels.join(", ")}`] : []),
    ...(task.blockedBy.length > 0 ? [`  Blocked by: ${task.blockedBy.join(", ")}`] : []),
    ...(task.blocks.length > 0 ? [`  Blocks: ${task.blocks.join(", ")}`] : []),
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

export function buildCompactReadyTaskPayload(page: ReadyTaskPage): CompactReadyTaskPayload {
  return {
    schemaVersion: 1,
    totalReady: page.totalReady,
    returned: page.items.length,
    hasMore: page.items.length < page.totalReady,
    items: page.items.map((task) => ({
      id: task.id,
      title: task.title,
      status: task.status,
      priority: task.priority,
      type: task.type,
      labels: task.labels,
      ...(task.parentId ? { parentId: task.parentId } : {}),
      ...(task.assignee ? { assignee: task.assignee } : {}),
    })),
  };
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
  if (task.claimedAt) lines.push(`  Claimed at: ${task.claimedAt}`);
  if (task.labels.length > 0) lines.push(`  Labels: ${task.labels.join(", ")}`);
  if (task.blockedBy.length > 0) lines.push(`  Blocked by: ${task.blockedBy.join(", ")}`);
  if (task.blocks.length > 0) lines.push(`  Blocks: ${task.blocks.join(", ")}`);
  if (task.closeReason) lines.push(`  Close reason: ${task.closeReason}`);
  if (task.receipt) {
    lines.push(`  Receipt:`);
    lines.push(`    Summary: ${task.receipt.summary}`);
    if (task.receipt.surprise) {
      lines.push(`    Surprise: ${task.receipt.surprise}`);
    }
    if (task.receipt.verifiedBy && task.receipt.verifiedBy.length > 0) {
      lines.push(`    Verified by: ${task.receipt.verifiedBy.join(", ")}`);
    }
    lines.push(`    Captured at: ${task.receipt.capturedAt}`);
  }

  return lines;
}

export function formatTaskShowView(view: TaskShowView): string[] {
  const lines = formatTaskDetail(view.task);
  const summary = view.continuation;
  if (!summary) {
    return lines;
  }

  lines.push(`  Last active: ${summary.lastActiveAt}`);
  if (summary.activeAgent) {
    const sessionSuffix = summary.activeAgent.sessionId ? `/${summary.activeAgent.sessionId}` : "";
    lines.push(`  Active agent: ${summary.activeAgent.type}${sessionSuffix}`);
  }
  lines.push(`  Current state: ${summary.currentState}`);
  lines.push(`  Next action: ${summary.nextAction}`);
  if (summary.keyDecisions.length > 0) {
    lines.push(`  Active decisions: ${summary.keyDecisions.join(" | ")}`);
  }

  if (view.recentEvents.length === 0) {
    lines.push(`  Recent timeline: no local timeline available`);
    return lines;
  }

  lines.push(`  Recent timeline:`);
  for (const event of view.recentEvents) {
    lines.push(`    - ${event.at} ${formatContinuationEvent(event)}`);
  }
  return lines;
}

function formatContinuationEvent(event: TaskShowView["recentEvents"][number]): string {
  switch (event.kind) {
    case "snapshot":
      return `snapshot: ${event.summary}`;
    case "decision":
      return `decision: ${event.summary}`;
    case "next_action_set":
      return `next action: ${event.summary}`;
    case "blocker_set":
      return `blocker change: ${event.summary}`;
    case "handoff_created":
      return `handoff created: ${event.handoffId} for ${event.agent}`;
    case "handoff_picked_up":
      return `handoff picked up: ${event.handoffId} by ${event.agent}`;
    case "agent_takeover":
      return `agent takeover: ${event.summary}`;
    case "task_completed":
      return `completed: ${event.summary}`;
    case "task_reopened":
      return `reopened: ${event.summary}`;
  }
}
