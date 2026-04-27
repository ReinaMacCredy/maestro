import type { Task } from "../domain/task-types.js";
import { hasUnresolvedBlockers, isTaskReady } from "../domain/task-state.js";

export interface TaskStatusHeader {
  readonly open: number;
  readonly active: number;
  readonly ready: number;
  readonly pending: number;
  readonly blocked: number;
  readonly blockedTracks: number;
}

export interface TaskTrackGroup {
  /** Display identifier: the slug when present, else the bare `tsk-<id>`. */
  readonly identifier: string;
  /** Slug when the track has one; undefined for slugless legacy top-level tasks. */
  readonly slug?: string;
  readonly task: Task;
  readonly steps: readonly Task[];
}

export interface TaskStatusProjection {
  readonly header: TaskStatusHeader;
  readonly tracks: readonly TaskTrackGroup[];
  /**
   * Top-level orphans: steps whose parent is missing. Rendered as a synthetic
   * "(orphans)" track so they aren't silently dropped (H2).
   */
  readonly orphans: readonly Task[];
  /**
   * Lookup of every task in the input (including completed) keyed by id. The
   * renderer uses this to resolve blocker slugs even when the blocker isn't
   * itself rendered (e.g. a completed track referenced by `blocked by`).
   */
  readonly tasksById: ReadonlyMap<string, Task>;
}

export interface GroupOptions {
  /** When true, include completed tasks. Default false (hide completed). */
  readonly includeCompleted?: boolean;
  /** Restrict to a single track by slug or `tsk-<id>` identifier. */
  readonly trackFilter?: string;
}

/**
 * Group tasks by their top-level "track" ancestor.
 *
 * H1: a step's grandparent (and beyond) chain rolls up to the nearest top-level
 *     ancestor; a 3-deep grandchild lands under the same track as its
 *     grandparent.
 * H2: steps with a missing parent become orphans (returned as `orphans`).
 * H3: a top-level task without a slug renders with its bare `tsk-<id>` as its
 *     identifier so 99 legacy tasks still display.
 * H4: tracks with zero steps still render their headline so the track is
 *     visible (relied on for empty in-progress tracks).
 * H5: parent cycles are already prevented at validate-time; the renderer
 *     trusts the input.
 */
export function groupTasksByTrack(
  tasks: readonly Task[],
  options: GroupOptions = {},
): TaskStatusProjection {
  const includeCompleted = options.includeCompleted === true;
  const byId = new Map(tasks.map((task) => [task.id, task] as const));

  const stepsByTrack = new Map<string, Task[]>();
  const orphans: Task[] = [];
  for (const task of tasks) {
    if (task.parentId === undefined) continue;
    const ancestor = nearestTrackAncestor(task, byId);
    if (ancestor === undefined) {
      if (!includeCompleted && task.status === "completed") continue;
      orphans.push(task);
      continue;
    }
    const list = stepsByTrack.get(ancestor.id);
    if (list) list.push(task);
    else stepsByTrack.set(ancestor.id, [task]);
  }

  const tracksRaw: TaskTrackGroup[] = [];
  for (const task of tasks) {
    if (task.parentId !== undefined) continue;
    const allSteps = stepsByTrack.get(task.id) ?? [];
    const stepsForDisplay = includeCompleted
      ? allSteps
      : allSteps.filter((step) => step.status !== "completed");
    if (!includeCompleted && task.status === "completed" && stepsForDisplay.length === 0) {
      continue;
    }
    const identifier = task.slug ?? task.id;
    if (
      options.trackFilter !== undefined &&
      options.trackFilter !== identifier &&
      options.trackFilter !== task.id
    ) {
      continue;
    }
    tracksRaw.push({
      identifier,
      slug: task.slug,
      task,
      steps: sortSteps(stepsForDisplay, byId),
    });
  }

  const tracks = sortTracks(tracksRaw);
  const headerTasks = options.trackFilter === undefined
    ? tasks
    : tracksRaw.flatMap((track) => [track.task, ...track.steps]);
  const header = computeHeader(headerTasks, byId);

  return {
    header,
    tracks,
    orphans: options.trackFilter === undefined ? orphans : [],
    tasksById: byId,
  };
}

function nearestTrackAncestor(
  task: Task,
  byId: ReadonlyMap<string, Task>,
): Task | undefined {
  let current: Task | undefined = task;
  const visited = new Set<string>();
  while (current !== undefined && current.parentId !== undefined) {
    if (visited.has(current.id)) return undefined;
    visited.add(current.id);
    current = byId.get(current.parentId);
  }
  return current;
}

function rank(status: Task["status"], blocked: boolean): number {
  if (status === "in_progress") return 0;
  if (blocked) return 1;
  if (status === "pending") return 2;
  if (status === "completed") return 3;
  return 4;
}

function isBlocked(task: Task, byId: ReadonlyMap<string, Task>): boolean {
  return hasUnresolvedBlockers(task, byId);
}

function sortSteps(
  steps: readonly Task[],
  byId: ReadonlyMap<string, Task>,
): Task[] {
  return [...steps].sort((a, b) => {
    const aRank = rank(a.status, isBlocked(a, byId));
    const bRank = rank(b.status, isBlocked(b, byId));
    if (aRank !== bRank) return aRank - bRank;
    if (a.priority !== b.priority) return a.priority - b.priority;
    return a.createdAt.localeCompare(b.createdAt);
  });
}

function sortTracks(tracks: readonly TaskTrackGroup[]): TaskTrackGroup[] {
  // Sort order, top to bottom:
  //   1. tracks the operator is actively driving (track-task `in_progress`),
  //   2. tracks with a slug (the modern "named" view),
  //   3. legacy slugless `tsk-XXX` tracks (so heavy backlogs of pre-slug
  //      tasks don't push the clean named view off-screen).
  // Within each bucket, ties break by createdAt so the on-disk layout drives
  // display and matches the agent-friendly screenshot fixture.
  return [...tracks].sort((a, b) => {
    const aActive = a.task.status === "in_progress" ? 0 : 1;
    const bActive = b.task.status === "in_progress" ? 0 : 1;
    if (aActive !== bActive) return aActive - bActive;
    const aSlugged = a.slug ? 0 : 1;
    const bSlugged = b.slug ? 0 : 1;
    if (aSlugged !== bSlugged) return aSlugged - bSlugged;
    return a.task.createdAt.localeCompare(b.task.createdAt);
  });
}

/**
 * Count the visible workload bucketed by the renderer's display rules:
 *
 * - every step is counted as its own status,
 * - top-level tracks with zero non-completed steps are counted as themselves,
 * - top-level tracks with steps are usually treated as containers and not
 *   counted, except they do count once toward `blocked` when the track
 *   aggregate is "blocked" (any blocked step, no in_progress step) so the
 *   header surfaces tracks that are stuck.
 */
function computeHeader(
  tasks: readonly Task[],
  byId: ReadonlyMap<string, Task>,
): TaskStatusHeader {
  let active = 0;
  let ready = 0;
  let pending = 0;
  let blocked = 0;
  let blockedTracks = 0;

  const childrenByParent = new Map<string, Task[]>();
  for (const task of tasks) {
    if (task.parentId === undefined) continue;
    const list = childrenByParent.get(task.parentId);
    if (list) list.push(task);
    else childrenByParent.set(task.parentId, [task]);
  }

  for (const task of tasks) {
    if (task.status === "completed") continue;
    if (task.parentId === undefined) {
      const visibleDescendants = collectVisibleDescendants(task.id, childrenByParent);
      if (visibleDescendants.length === 0) {
        if (task.status === "in_progress") active += 1;
        else if (isBlocked(task, byId)) {
          blocked += 1;
          blockedTracks += 1;
        } else {
          pending += 1;
          if (task.assignee === undefined && isTaskReady(task, byId)) ready += 1;
        }
        continue;
      }
      const trackHasInProgress = visibleDescendants.some(
        (child) => child.status === "in_progress",
      );
      const trackHasBlocked = visibleDescendants.some((child) => isBlocked(child, byId));
      if (trackHasBlocked) blockedTracks += 1;
      if (!trackHasInProgress && trackHasBlocked) blocked += 1;
      continue;
    }

    if (task.status === "in_progress") active += 1;
    else if (isBlocked(task, byId)) blocked += 1;
    else {
      pending += 1;
      if (task.assignee === undefined && isTaskReady(task, byId)) ready += 1;
    }
  }
  return {
    open: active + pending + blocked,
    active,
    ready,
    pending,
    blocked,
    blockedTracks,
  };
}

function collectVisibleDescendants(
  taskId: string,
  childrenByParent: ReadonlyMap<string, readonly Task[]>,
): readonly Task[] {
  const visible: Task[] = [];
  const stack = [...(childrenByParent.get(taskId) ?? [])].reverse();
  while (stack.length > 0) {
    const child = stack.pop()!;
    if (child.status !== "completed") {
      visible.push(child);
    }
    const children = childrenByParent.get(child.id) ?? [];
    for (let idx = children.length - 1; idx >= 0; idx--) {
      stack.push(children[idx]!);
    }
  }
  return visible;
}
