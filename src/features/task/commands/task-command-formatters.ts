import { formatRelativeAge } from "@/shared/version-format.js";
import { colorize, isColorEnabled } from "@/shared/lib/ansi.js";
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

const GLYPH = {
  active: "o",
  blocked: "!",
  done: "v",
  pending: "·",
} as const;

const ACTIVE_SECTION_LIMIT = 4;
const LIST_SECTION_LIMIT = 5;
const DEPENDENCY_TRACK_COMPACT_LIMIT = 3;
const DEPENDENCY_TRACK_FULL_LIMIT = 4;

function trackIdentifier(task: Pick<Task, "id" | "slug" | "parentId">): string {
  return task.parentId === undefined && task.slug ? task.slug : task.id;
}

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
  return [
    `[ok] Task created: ${trackIdentifier(task)}`,
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
    lines.push(`${trackIdentifier(task)}  ${priority}  ${status}  ${title}`);
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
    lines.push(`${trackIdentifier(briefing)}  ${priority}  ${status}  ${title}`);
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
  /**
   * When false, render the unsectioned grouped detail view. Default true renders
   * the operator board.
   */
  readonly compact?: boolean;
}

/**
 * Render the `task status` track board from a projection. The default view
 * keeps each track in one visual block so operators can scan active, ready, and
 * blocked steps without mentally joining sections.
 */
export function formatTaskStatusView(
  projection: TaskStatusProjection,
  opts: FormatTaskStatusOptions = {},
): string[] {
  const colorOn = opts.color ?? isColorEnabled();
  const { header, tracks, orphans, tasksById } = projection;

  if (opts.compact === false) {
    return formatGroupedTaskStatusView(projection, colorOn);
  }

  return formatTrackBoardTaskStatusView(projection, colorOn, opts.all === true);
}

function formatTrackBoardTaskStatusView(
  projection: TaskStatusProjection,
  colorOn: boolean,
  showCompleted: boolean,
): string[] {
  const { header, tracks, orphans, tasksById } = projection;
  const lines: string[] = [formatTaskStatusHeader(header)];

  if (tracks.length === 0 && orphans.length === 0) {
    return lines;
  }

  const items = collectStatusItems(tracks, tasksById);
  const downstreamBlockedCounts = collectDownstreamBlockedCounts(tasksById);
  const nextLine = formatNextReadyLine(items, downstreamBlockedCounts, colorOn);
  if (nextLine !== undefined) {
    lines.push(nextLine);
  }

  const dependencyTracks = tracks.filter((track) =>
    isDependencyTrack(track, tasksById, downstreamBlockedCounts),
  );
  const dependencyTrackIds = new Set(dependencyTracks.map((track) => track.task.id));
  const simpleItems = items.filter((item) => !dependencyTrackIds.has(item.track.task.id));

  appendInlineItemSection(
    lines,
    "ACTIVE",
    simpleItems.filter((item) => item.task.status === "in_progress"),
    tasksById,
    downstreamBlockedCounts,
    colorOn,
    ACTIVE_SECTION_LIMIT,
    "title",
  );
  appendDependencyTracksSection(
    lines,
    dependencyTracks,
    tasksById,
    downstreamBlockedCounts,
    colorOn,
  );
  appendInlineItemSection(
    lines,
    "READY",
    simpleItems.filter(isReadyStatusItem),
    tasksById,
    downstreamBlockedCounts,
    colorOn,
    LIST_SECTION_LIMIT,
    "title",
  );
  appendInlineItemSection(
    lines,
    "BLOCKED",
    simpleItems.filter((item) => item.blocked),
    tasksById,
    downstreamBlockedCounts,
    colorOn,
    LIST_SECTION_LIMIT,
    "blockedBy",
  );
  if (showCompleted) {
    appendInlineItemSection(
      lines,
      "DONE",
      simpleItems.filter((item) => item.task.status === "completed"),
      tasksById,
      downstreamBlockedCounts,
      colorOn,
      LIST_SECTION_LIMIT,
      "title",
    );
  }

  if (orphans.length > 0) {
    lines.push("");
    lines.push(colorize("ORPHANS", "dim", colorOn));
    for (const orphan of orphans) {
      appendStep(lines, orphan, tasksById, colorOn);
    }
  }

  return lines;
}

function appendDependencyTracksSection(
  lines: string[],
  tracks: readonly TaskTrackGroup[],
  tasksById: ReadonlyMap<string, Task>,
  downstreamBlockedCounts: ReadonlyMap<string, number>,
  colorOn: boolean,
): void {
  if (tracks.length === 0) return;
  lines.push("");
  lines.push(colorize("DEPENDENCY TRACKS", "dim", colorOn));

  for (const track of tracks) {
    const trackItems = sortTrackBoardItems(
      collectTrackItems(track, tasksById),
      downstreamBlockedCounts,
    );
    if (trackItems.length === 0) continue;
    lines.push("");
    lines.push(colorize(track.identifier, "cyan", colorOn));
    const limit = dependencyTrackItemLimit(trackItems.length);
    for (const item of trackItems.slice(0, limit)) {
      appendTrackBoardItem(lines, item, tasksById, downstreamBlockedCounts, colorOn);
    }
    appendMoreLine(lines, trackItems.length - limit);
  }
}

function dependencyTrackItemLimit(itemCount: number): number {
  return itemCount > DEPENDENCY_TRACK_FULL_LIMIT
    ? DEPENDENCY_TRACK_COMPACT_LIMIT
    : DEPENDENCY_TRACK_FULL_LIMIT;
}

function formatTaskStatusHeader(header: TaskStatusProjection["header"]): string {
  return [
    `tasks: ${header.open} open`,
    `${header.active} active`,
    `${header.ready} ready`,
    `${header.blocked} blocked`,
    `${header.blockedTracks} ${pluralize("blocked track", header.blockedTracks)}`,
  ].join(" | ");
}

function formatNextReadyLine(
  items: readonly StatusItem[],
  downstreamBlockedCounts: ReadonlyMap<string, number>,
  colorOn: boolean,
): string | undefined {
  const ready = items.filter(isReadyStatusItem);
  if (ready.length === 0) return undefined;
  const [best] = [...ready].sort((a, b) => {
    const aDownstream = downstreamCount(a.task, downstreamBlockedCounts);
    const bDownstream = downstreamCount(b.task, downstreamBlockedCounts);
    if (aDownstream !== bDownstream) return bDownstream - aDownstream;
    if (a.task.priority !== b.task.priority) return a.task.priority - b.task.priority;
    return a.task.createdAt.localeCompare(b.task.createdAt);
  });
  if (best === undefined) return undefined;
  const downstream = downstreamCount(best.task, downstreamBlockedCounts);
  if (downstream === 0) return undefined;
  return colorize(
    `next: ${best.track.identifier} / ${best.task.title} (${pluralizeCount(downstream, "unblock")})`,
    "dim",
    colorOn,
  );
}

function formatGroupedTaskStatusView(
  projection: TaskStatusProjection,
  colorOn: boolean,
): string[] {
  const { header, tracks, orphans, tasksById } = projection;
  const lines: string[] = [
    `tasks: ${header.active} active, ${header.pending} pending, ${header.blocked} blocked`,
  ];

  if (tracks.length === 0 && orphans.length === 0) {
    return lines;
  }

  let prevSolo = false;
  for (const track of tracks) {
    const isSolo = track.steps.length === 0;
    if (!(prevSolo && isSolo)) {
      lines.push("");
    }
    if (isSolo) {
      lines.push(formatSoloTrackLine(track, tasksById, colorOn));
    } else {
      appendTrack(lines, track, tasksById, colorOn);
    }
    prevSolo = isSolo;
  }

  if (orphans.length > 0) {
    lines.push("");
    lines.push(colorize("(orphans)", "dim", colorOn));
    for (const orphan of orphans) {
      appendStep(lines, orphan, tasksById, colorOn);
    }
  }

  return lines;
}

interface StatusItem {
  readonly track: TaskTrackGroup;
  readonly task: Task;
  readonly blocked: boolean;
}

function collectStatusItems(
  tracks: readonly TaskTrackGroup[],
  tasksById: ReadonlyMap<string, Task>,
): StatusItem[] {
  return tracks.flatMap((track) => collectTrackItems(track, tasksById));
}

function collectTrackItems(
  track: TaskTrackGroup,
  tasksById: ReadonlyMap<string, Task>,
): StatusItem[] {
  const tasks = track.steps.length > 0 ? track.steps : [track.task];
  return tasks.map((task) => ({
    track,
    task,
    blocked: hasUnresolvedBlockers(task, tasksById),
  }));
}

function isDependencyTrack(
  track: TaskTrackGroup,
  tasksById: ReadonlyMap<string, Task>,
  downstreamBlockedCounts: ReadonlyMap<string, number>,
): boolean {
  if (track.steps.length === 0) return false;
  return collectTrackItems(track, tasksById).some((item) =>
    item.blocked || downstreamCount(item.task, downstreamBlockedCounts) > 0,
  );
}

function appendInlineItemSection(
  lines: string[],
  title: string,
  items: readonly StatusItem[],
  tasksById: ReadonlyMap<string, Task>,
  downstreamBlockedCounts: ReadonlyMap<string, number>,
  colorOn: boolean,
  limit: number,
  mode: "title" | "blockedBy",
): void {
  if (items.length === 0) return;
  lines.push("");
  lines.push(colorize(title, "dim", colorOn));
  const visible = items.slice(0, limit);
  const identifierWidth = Math.max(...visible.map((item) => item.track.identifier.length));
  for (const item of visible) {
    appendInlineItem(
      lines,
      item,
      tasksById,
      downstreamBlockedCounts,
      colorOn,
      identifierWidth,
      mode,
    );
  }
  appendMoreLine(lines, items.length - limit);
}

function appendInlineItem(
  lines: string[],
  item: StatusItem,
  tasksById: ReadonlyMap<string, Task>,
  downstreamBlockedCounts: ReadonlyMap<string, number>,
  colorOn: boolean,
  identifierWidth: number,
  mode: "title" | "blockedBy",
): void {
  const glyph = stepGlyph(item.task, item.blocked, colorOn);
  const identifier = colorize(item.track.identifier.padEnd(identifierWidth), "cyan", colorOn);
  if (mode === "blockedBy") {
    const blockedBy = formatBlockedByLine(item.task, tasksById, colorOn);
    lines.push(`  ${glyph} ${identifier}  ${blockedBy}`);
    return;
  }
  const downstream = downstreamCount(item.task, downstreamBlockedCounts);
  const suffix = downstream > 0 ? `  ${pluralizeCount(downstream, "unblock")}` : "";
  lines.push(`  ${glyph} ${identifier}  ${item.task.title}${suffix}`);
}

function appendTrackBoardItem(
  lines: string[],
  item: StatusItem,
  tasksById: ReadonlyMap<string, Task>,
  downstreamBlockedCounts: ReadonlyMap<string, number>,
  colorOn: boolean,
): void {
  lines.push(`  ${stepGlyph(item.task, item.blocked, colorOn)} ${item.task.title}`);
  if (item.task.status === "in_progress") {
    lines.push(`      ${colorize("in-progress", "yellow", colorOn)}`);
    return;
  }
  if (item.blocked) {
    lines.push(`      ${formatBlockedByLine(item.task, tasksById, colorOn)}`);
    return;
  }
  if (isReadyStatusItem(item)) {
    const downstream = downstreamCount(item.task, downstreamBlockedCounts);
    if (downstream > 0) {
      lines.push(`      ready, ${pluralizeCount(downstream, "unblock")}`);
    }
  }
}

function sortTrackBoardItems(
  items: readonly StatusItem[],
  downstreamBlockedCounts: ReadonlyMap<string, number>,
): StatusItem[] {
  return [...items].sort((a, b) => {
    const aRank = trackBoardItemRank(a, downstreamBlockedCounts);
    const bRank = trackBoardItemRank(b, downstreamBlockedCounts);
    if (aRank !== bRank) return aRank - bRank;
    const aDownstream = downstreamCount(a.task, downstreamBlockedCounts);
    const bDownstream = downstreamCount(b.task, downstreamBlockedCounts);
    if (aDownstream !== bDownstream) return bDownstream - aDownstream;
    if (a.task.priority !== b.task.priority) return a.task.priority - b.task.priority;
    return a.task.createdAt.localeCompare(b.task.createdAt);
  });
}

function trackBoardItemRank(
  item: StatusItem,
  downstreamBlockedCounts: ReadonlyMap<string, number>,
): number {
  if (item.task.status === "in_progress") return 0;
  if (isReadyStatusItem(item) && downstreamCount(item.task, downstreamBlockedCounts) > 0) {
    return 1;
  }
  if (item.blocked) return 2;
  if (isReadyStatusItem(item)) return 3;
  if (item.task.status === "pending") return 4;
  if (item.task.status === "completed") return 5;
  return 6;
}

function appendMoreLine(lines: string[], hiddenCount: number): void {
  if (hiddenCount > 0) {
    lines.push(`  + ${hiddenCount} more`);
  }
}

function isReadyStatusItem(item: StatusItem): boolean {
  return (
    item.task.status === "pending" &&
    item.task.assignee === undefined &&
    !item.blocked
  );
}

function pluralize(label: string, count: number): string {
  return count === 1 ? label : `${label}s`;
}

function pluralizeCount(count: number, label: string): string {
  return `${count} ${count === 1 ? label : `${label}s`}`;
}

function collectDownstreamBlockedCounts(
  tasksById: ReadonlyMap<string, Task>,
): ReadonlyMap<string, number> {
  const waitingByBlocker = new Map<string, Task[]>();
  for (const candidate of tasksById.values()) {
    if (candidate.status === "completed") continue;
    for (const blockerId of candidate.blockedBy) {
      const list = waitingByBlocker.get(blockerId);
      if (list) list.push(candidate);
      else waitingByBlocker.set(blockerId, [candidate]);
    }
  }

  const counts = new Map<string, number>();
  for (const task of tasksById.values()) {
    const seen = new Set<string>();
    const queue = [...(waitingByBlocker.get(task.id) ?? [])];
    while (queue.length > 0) {
      const current = queue.shift();
      if (current === undefined || seen.has(current.id)) continue;
      seen.add(current.id);
      queue.push(...(waitingByBlocker.get(current.id) ?? []));
    }
    if (seen.size > 0) counts.set(task.id, seen.size);
  }
  return counts;
}

function downstreamCount(
  task: Task,
  downstreamBlockedCounts: ReadonlyMap<string, number>,
): number {
  return downstreamBlockedCounts.get(task.id) ?? 0;
}

function appendTrack(
  lines: string[],
  track: TaskTrackGroup,
  tasksById: ReadonlyMap<string, Task>,
  colorOn: boolean,
): void {
  lines.push(colorize(track.identifier, "cyan", colorOn));
  for (const step of track.steps) {
    appendStep(lines, step, tasksById, colorOn);
  }
}

function formatSoloTrackLine(
  track: TaskTrackGroup,
  tasksById: ReadonlyMap<string, Task>,
  colorOn: boolean,
): string {
  const task = track.task;
  const blocked = hasUnresolvedBlockers(task, tasksById);
  const glyph = stepGlyph(task, blocked, colorOn);
  const slug = colorize(track.identifier, "cyan", colorOn);
  const status = stepStatusLine(task, blocked, tasksById, colorOn);
  const head = `  ${glyph} ${slug}  ${task.title}`;
  return status === undefined ? head : `${head}  ${status}`;
}

function appendStep(
  lines: string[],
  task: Task,
  tasksById: ReadonlyMap<string, Task>,
  colorOn: boolean,
): void {
  const blocked = hasUnresolvedBlockers(task, tasksById);
  lines.push(`  ${stepGlyph(task, blocked, colorOn)} ${task.title}`);
  const status = stepStatusLine(task, blocked, tasksById, colorOn);
  if (status !== undefined) {
    lines.push(`      ${status}`);
  }
}

function stepGlyph(task: Task, blocked: boolean, colorOn: boolean): string {
  if (task.status === "in_progress") return colorize(GLYPH.active, "green", colorOn);
  if (blocked) return colorize(GLYPH.blocked, "yellow", colorOn);
  if (task.status === "completed") return GLYPH.done;
  return GLYPH.pending;
}

function stepStatusLine(
  task: Task,
  blocked: boolean,
  tasksById: ReadonlyMap<string, Task>,
  colorOn: boolean,
): string | undefined {
  if (task.status === "in_progress") {
    return colorize("in-progress", "yellow", colorOn);
  }
  if (task.status !== "pending" || !blocked) return undefined;

  return formatBlockedByLine(task, tasksById, colorOn);
}

function formatBlockedByLine(
  task: Task,
  tasksById: ReadonlyMap<string, Task>,
  colorOn: boolean,
): string {
  const labels = task.blockedBy.map((id) => describeBlocker(id, tasksById));
  return colorize(`blocked by ${labels.join(", ")}`, "yellow", colorOn);
}

function describeBlocker(blockerId: string, tasksById: ReadonlyMap<string, Task>): string {
  const blocker = tasksById.get(blockerId);
  if (!blocker) return blockerId;
  const label = blocker.parentId === undefined ? trackIdentifier(blocker) : blocker.title;
  return blocker.status === "completed" ? `${label} (done)` : label;
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
