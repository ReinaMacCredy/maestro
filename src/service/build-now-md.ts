import type { Task } from "../types/task.js";
import type { TaskState } from "../types/task-state.js";
import { formatRelativeAge } from "../shared/version-format.js";

export const STUCK_THRESHOLD_MS = 4 * 60 * 60 * 1000;

const READY_LIMIT = 5;

const IN_FLIGHT_STATES = ["claimed", "doing", "verifying"] as const satisfies readonly TaskState[];

function isInFlight(task: Task): boolean {
  return (IN_FLIGHT_STATES as readonly TaskState[]).includes(task.state);
}

export function isStuck(task: Task, now: Date, thresholdMs: number = STUCK_THRESHOLD_MS): boolean {
  if (!isInFlight(task)) return false;
  const updated = Date.parse(task.updated_at);
  if (Number.isNaN(updated)) return false;
  return now.getTime() - updated > thresholdMs;
}

export interface BuildNowMdInput {
  readonly tasks: readonly Task[];
  readonly now: Date;
}

export function buildNowMd({ tasks, now }: BuildNowMdInput): string {
  const updated = now.toISOString();

  if (tasks.length === 0) {
    return `# NOW\nUpdated: ${updated}\n\nNo tasks yet.\n`;
  }

  const inFlight = tasks.filter(isInFlight).sort(byClaimedAtThenId);

  const readyAll = tasks
    .filter((t) => t.state === "draft" && t.blocked_by.length === 0)
    .sort(byCreatedAt);
  const ready = readyAll.slice(0, READY_LIMIT);

  const readyToShip = tasks.filter((t) => t.state === "ready").sort(byUpdatedAt);
  const blocked = tasks.filter((t) => t.state === "blocked").sort(byCreatedAt);

  // Subset of inFlight; rendered again as an attention digest so agents
  // landing on NOW.md see idle work without scanning the In flight list.
  const stuck = inFlight.filter((t) => isStuck(t, now));

  const lines: string[] = [];
  lines.push("# NOW");
  lines.push(`Updated: ${updated}`);
  lines.push("");

  lines.push(`## In flight (${inFlight.length})`);
  if (inFlight.length === 0) {
    lines.push("None.");
  } else {
    for (const t of inFlight) lines.push(...renderInFlight(t, now));
  }
  lines.push("");

  lines.push(`## Ready to pick up (${readyAll.length})`);
  if (readyAll.length === 0) {
    lines.push("None.");
  } else {
    for (const t of ready) lines.push(...renderDraft(t));
    if (readyAll.length > READY_LIMIT) {
      lines.push(`(and ${readyAll.length - READY_LIMIT} more)`);
      lines.push("");
    }
  }
  lines.push("");

  lines.push(`## Ready to ship (${readyToShip.length})`);
  if (readyToShip.length === 0) {
    lines.push("None.");
  } else {
    for (const t of readyToShip) lines.push(...renderReady(t));
  }
  lines.push("");

  lines.push(`## Blocked (${blocked.length})`);
  if (blocked.length === 0) {
    lines.push("None.");
  } else {
    for (const t of blocked) lines.push(...renderBlocked(t));
  }
  lines.push("");

  lines.push(`## Stuck (${stuck.length})`);
  if (stuck.length === 0) {
    lines.push("None.");
  } else {
    for (const t of stuck) lines.push(...renderStuck(t, now));
  }

  return lines.join("\n") + "\n";
}

function renderInFlight(task: Task, now: Date): readonly string[] {
  const out: string[] = [];
  out.push(`### ${task.id} . ${task.title}`);
  out.push(`State: ${task.state}`);
  if (task.assignee || task.claimed_at) {
    const owner = task.assignee ?? "unassigned";
    const claimed = task.claimed_at ? formatRelativeAge(task.claimed_at, now) : "unknown";
    out.push(`Owner: ${owner} (claimed ${claimed})`);
  }
  if (task.plan_id) out.push(`Plan: ${task.plan_id}`);
  if (task.worktree_path) out.push(`Worktree: ${task.worktree_path}`);
  out.push("");
  return out;
}

function renderDraft(task: Task): readonly string[] {
  const out: string[] = [];
  out.push(`### ${task.id} . ${task.title}`);
  if (task.plan_id) out.push(`Plan: ${task.plan_id}`);
  if (task.spec_path) out.push(`Spec: ${task.spec_path}`);
  out.push("");
  return out;
}

function renderReady(task: Task): readonly string[] {
  const out: string[] = [];
  out.push(`### ${task.id} . ${task.title}`);
  if (task.assignee) out.push(`Owner: ${task.assignee}`);
  if (task.pr_url) out.push(`PR: ${task.pr_url}`);
  out.push("");
  return out;
}

function renderBlocked(task: Task): readonly string[] {
  const out: string[] = [];
  out.push(`### ${task.id} . ${task.title}`);
  if (task.block_reason) out.push(`Reason: ${task.block_reason}`);
  if (task.blocked_by.length > 0) {
    out.push(`Blocked by: ${task.blocked_by.join(", ")}`);
  }
  out.push("");
  return out;
}

function renderStuck(task: Task, now: Date): readonly string[] {
  const out: string[] = [];
  out.push(`### ${task.id} . ${task.title}`);
  out.push(`State: ${task.state} (last update ${formatRelativeAge(task.updated_at, now)})`);
  if (task.assignee) out.push(`Owner: ${task.assignee}`);
  out.push("");
  return out;
}

function byClaimedAtThenId(a: Task, b: Task): number {
  const aClaimed = a.claimed_at ?? "";
  const bClaimed = b.claimed_at ?? "";
  if (aClaimed !== bClaimed) return aClaimed.localeCompare(bClaimed);
  return a.id.localeCompare(b.id);
}

function byCreatedAt(a: Task, b: Task): number {
  return a.created_at.localeCompare(b.created_at);
}

function byUpdatedAt(a: Task, b: Task): number {
  return a.updated_at.localeCompare(b.updated_at);
}
