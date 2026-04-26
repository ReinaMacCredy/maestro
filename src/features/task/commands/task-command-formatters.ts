import { formatRelativeAge } from "@/shared/version-format.js";
import { colorize, isColorEnabled } from "@/shared/lib/ansi.ts";
import type { Task } from "../domain/task-types.js";
import type { TaskShowView } from "../usecases/task-continuation.usecase.js";
import type { TaskHint } from "../usecases/match-candidates.usecase.js";
import type { ReadyTaskPage, TaskBriefing } from "../usecases/ready-tasks.usecase.js";
import type {
  PruneKindReport,
  PruneReport,
} from "../usecases/prune-local-task-state.usecase.js";
import type {
  TaskStatusProjection,
  TaskTrackGroup,
} from "../usecases/group-tasks-by-track.usecase.js";
import { hasUnresolvedBlockers } from "../domain/task-state.js";

export type CompactReadyTaskItem = Pick<
  Task,
  "id" | "title" | "status" | "priority" | "type" | "labels" | "parentId" | "assignee"
> & { readonly slug?: string };

export interface CompactReadyTaskPayload {
  readonly schemaVersion: 1;
  readonly totalReady: number;
  readonly returned: number;
  readonly hasMore: boolean;
  readonly items: readonly CompactReadyTaskItem[];
}

export function formatTaskSummary(task: Task): string[] {
  const headerLabel = task.parentId === undefined && task.slug ? task.slug : task.id;
  return [
    `[ok] Task created: ${headerLabel}`,
    ...(task.slug ? [`  Slug: ${task.slug}`] : []),
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
    const identifier = task.parentId === undefined && task.slug ? task.slug : task.id;
    lines.push(`${identifier}  ${priority}  ${status}  ${title}`);
  }
  return lines;
}

/**
 * Render only the track headers from a task list ("--tracks" flag on `task
 * list`). Top-level slug tasks render their slug; legacy slugless top-level
 * tasks render their `tsk-<id>`. Step tasks are skipped.
 */
export function formatTaskTrackList(tasks: readonly Task[]): string[] {
  const tracks = tasks.filter((task) => task.parentId === undefined);
  if (tracks.length === 0) {
    return ["No tracks found"];
  }
  return tracks.map((task) => task.slug ?? task.id);
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
    const briefingWithSlug = briefing as TaskBriefing & { readonly slug?: string; readonly parentId?: string };
    const identifier = briefingWithSlug.parentId === undefined && briefingWithSlug.slug
      ? briefingWithSlug.slug
      : briefing.id;
    lines.push(`${identifier}  ${priority}  ${status}  ${title}`);
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
      ...(task.slug ? { slug: task.slug } : {}),
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
    ...(task.slug ? [`  Slug: ${task.slug}`] : []),
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
  if (view.steps && view.steps.length > 0) {
    lines.push(`  Steps:`);
    for (const step of view.steps) {
      lines.push(`    ${step.id}  ${step.status.padEnd(12)}  ${step.title}`);
    }
  }
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

export function formatPruneReport(report: PruneReport): string[] {
  const lines: string[] = [formatPruneHeader(report)];
  if (report.kinds !== "continuations") {
    lines.push(`  candidates: ${formatPruneKindLine(report.candidates, report.dryRun)}`);
  }
  if (report.kinds !== "candidates") {
    lines.push(`  continuations/completed: ${formatPruneKindLine(report.continuations, report.dryRun)}`);
  }
  return lines;
}

function formatPruneHeader(report: PruneReport): string {
  const prefix = report.dryRun ? "[dry-run] Would" : "[ok]";
  if (report.all) {
    const verb = report.dryRun ? "purge" : "Purged";
    return `${prefix} ${verb} all local task state (kinds: ${report.kinds})`;
  }
  const verb = report.dryRun ? "prune" : "Pruned";
  return `${prefix} ${verb} local task state (keep ${report.keep} per kind, kinds: ${report.kinds})`;
}

function formatPruneKindLine(kind: PruneKindReport, dryRun: boolean): string {
  const verb = dryRun ? "would purge" : "purged";
  const parts = [`${verb} ${kind.purged}`, `kept ${kind.kept}`];
  if (kind.oldestKeptAt) parts.push(`oldest kept ${kind.oldestKeptAt}`);
  if (kind.newestPurgedAt) parts.push(`newest purged ${kind.newestPurgedAt}`);
  return parts.join(", ");
}

export interface FormatTaskStatusOptions {
  /** When true, include completed tasks in the rendered output. */
  readonly all?: boolean;
  /** When true, emit ANSI color codes. Default: detect via NO_COLOR + TTY. */
  readonly color?: boolean;
}

/**
 * Render the screenshot-style `task status` view from a projection. Returns a
 * line array (no trailing newline). Plain-text shape is stable; color is only
 * applied when `opts.color` is explicitly true (default: auto-detect via
 * `isColorEnabled()`).
 */
export function formatTaskStatusView(
  projection: TaskStatusProjection,
  opts: FormatTaskStatusOptions = {},
): string[] {
  const colorOn = opts.color ?? isColorEnabled();
  const all = opts.all === true;
  const { header, tracks, orphans } = projection;

  const lines: string[] = [];
  lines.push(
    `tasks: ${header.active} active, ${header.pending} pending, ${header.blocked} blocked`,
  );

  if (tracks.length === 0 && orphans.length === 0) {
    return lines;
  }

  const tasksById = projection.tasksById;

  for (let idx = 0; idx < tracks.length; idx++) {
    const track = tracks[idx]!;
    lines.push("");
    appendTrack(lines, track, tasksById, colorOn, all);
  }

  if (orphans.length > 0) {
    lines.push("");
    lines.push(colorize("(orphans)", "dim", colorOn));
    for (const orphan of orphans) {
      const glyph = stepGlyph(orphan, tasksById, colorOn);
      lines.push(`  ${glyph} ${orphan.title}`);
      const status = stepStatusLine(orphan, tasksById, colorOn);
      if (status !== undefined) {
        lines.push(`      ${status}`);
      }
    }
  }

  return lines;
}

function appendTrack(
  lines: string[],
  track: TaskTrackGroup,
  tasksById: ReadonlyMap<string, Task>,
  colorOn: boolean,
  includeAll: boolean,
): void {
  lines.push(colorize(track.identifier, "cyan", colorOn));

  const visibleSteps = includeAll
    ? track.steps
    : track.steps.filter((step) => step.status !== "completed");

  // Tracks with no visible steps render the track-task itself as the single
  // bullet (H4). Tracks with steps treat the track-task as a container and
  // skip its title; only the steps render.
  if (visibleSteps.length === 0) {
    if (track.task.status === "completed" && !includeAll) return;
    const taskBlocked = hasUnresolvedBlockers(track.task, tasksById);
    const headlineGlyph = trackHeadlineGlyph(track.task, taskBlocked, colorOn);
    lines.push(`  ${headlineGlyph} ${track.task.title}`);
    const headlineStatus = stepStatusLine(track.task, tasksById, colorOn);
    if (headlineStatus !== undefined) {
      lines.push(`      ${headlineStatus}`);
    }
    return;
  }

  for (const step of visibleSteps) {
    const glyph = stepGlyph(step, tasksById, colorOn);
    lines.push(`  ${glyph} ${step.title}`);
    const status = stepStatusLine(step, tasksById, colorOn);
    if (status !== undefined) {
      lines.push(`      ${status}`);
    }
  }
}

function trackHeadlineGlyph(task: Task, blocked: boolean, colorOn: boolean): string {
  if (task.status === "in_progress") return colorize("o", "green", colorOn);
  if (blocked) return colorize("!", "red", colorOn);
  if (task.status === "completed") return "v";
  return "·";
}

function stepGlyph(
  step: Task,
  tasksById: ReadonlyMap<string, Task>,
  colorOn: boolean,
): string {
  if (step.status === "in_progress") return colorize("o", "green", colorOn);
  if (hasUnresolvedBlockers(step, tasksById)) return colorize("!", "red", colorOn);
  if (step.status === "completed") return "v";
  return "·";
}

function stepStatusLine(
  step: Task,
  tasksById: ReadonlyMap<string, Task>,
  colorOn: boolean,
): string | undefined {
  if (step.status === "in_progress") {
    return colorize("in-progress", "yellow", colorOn);
  }
  if (step.status === "pending") {
    if (!hasUnresolvedBlockers(step, tasksById)) {
      return undefined;
    }
    const labels = step.blockedBy
      .map((blockerId) => describeBlocker(blockerId, tasksById))
      .filter((label): label is string => label !== undefined);
    if (labels.length === 0) return undefined;
    return colorize(`blocked by ${labels.join(", ")}`, "red", colorOn);
  }
  return undefined;
}

function describeBlocker(
  blockerId: string,
  tasksById: ReadonlyMap<string, Task>,
): string | undefined {
  const blocker = tasksById.get(blockerId);
  if (!blocker) return blockerId;
  const label = blocker.parentId === undefined && blocker.slug ? blocker.slug : blocker.id;
  if (blocker.status === "completed") {
    return `${label} (done)`;
  }
  return label;
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
