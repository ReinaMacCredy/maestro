import type { Task } from "../domain/task-types.js";
import { indexTasksById } from "../domain/task-types.js";
import { hasUnresolvedBlockers } from "../domain/task-state.js";

const READY_LIMIT = 5;
const STUCK_THRESHOLD_MS = 4 * 60 * 60 * 1000;
const DESCRIPTION_TRUNCATE = 300;

export interface WriteNowMdInput {
  readonly tasks: readonly Task[];
  readonly now: Date;
}

export function buildNowMd({ tasks, now }: WriteNowMdInput): string {
  const updated = now.toISOString();

  if (tasks.length === 0) {
    return `# NOW\nUpdated: ${updated}\n\nNo tasks yet.\n`;
  }

  const byId = indexTasksById(tasks);
  const inProgress = tasks
    .filter((task) => task.status === "in_progress")
    .slice()
    .sort(byPriorityThenCreated);

  const ready = tasks
    .filter((task) => task.status === "pending" && !hasUnresolvedBlockers(task, byId))
    .slice()
    .sort(byPriorityThenCreated)
    .slice(0, READY_LIMIT);

  const stuck = inProgress.filter((task) => isStuck(task, now));

  const lines: string[] = [];
  lines.push("# NOW");
  lines.push(`Updated: ${updated}`);
  lines.push("");

  lines.push(`## In progress (${inProgress.length})`);
  if (inProgress.length === 0) {
    lines.push("None.");
  } else {
    for (const task of inProgress) {
      lines.push(...renderTask(task, now, byId, { includeOwner: true }));
    }
  }
  lines.push("");

  lines.push(`## Ready to pick up (${ready.length})`);
  if (ready.length === 0) {
    lines.push("None.");
  } else {
    for (const task of ready) {
      lines.push(...renderTask(task, now, byId, { includeOwner: false }));
    }
  }
  lines.push("");

  lines.push(`## Stuck (${stuck.length})`);
  if (stuck.length === 0) {
    lines.push("None.");
  } else {
    for (const task of stuck) {
      lines.push(...renderTask(task, now, byId, { includeOwner: true }));
    }
  }

  return lines.join("\n") + "\n";
}

function renderTask(
  task: Task,
  now: Date,
  byId: ReadonlyMap<string, Task>,
  opts: { includeOwner: boolean },
): readonly string[] {
  const out: string[] = [];
  out.push(`### ${task.id} . ${task.title}`);

  if (opts.includeOwner && task.assignee) {
    const claimed = task.claimedAt ? relative(task.claimedAt, now) : "unknown";
    const activity = relative(task.updatedAt, now);
    out.push(`Owner: ${task.assignee} (claimed ${claimed}, last activity ${activity})`);
  }

  out.push(`Priority: P${task.priority} | Type: ${task.type}`);

  if (task.labels.length > 0) {
    out.push(`Labels: ${task.labels.join(", ")}`);
  }

  if (task.blockedBy.length > 0) {
    const blockers = task.blockedBy
      .map((id) => {
        const blocker = byId.get(id);
        return blocker ? `${id} (${blocker.status})` : id;
      })
      .join(", ");
    out.push(`Blocked by: ${blockers}`);
  }

  const description = task.description?.trim();
  if (description && description.length > 0) {
    const truncated = description.length > DESCRIPTION_TRUNCATE
      ? description.slice(0, DESCRIPTION_TRUNCATE) + "..."
      : description;
    out.push(truncated);
  }

  out.push("");
  return out;
}

function byPriorityThenCreated(a: Task, b: Task): number {
  if (a.priority !== b.priority) return a.priority - b.priority;
  return a.createdAt.localeCompare(b.createdAt);
}

function isStuck(task: Task, now: Date): boolean {
  const updated = Date.parse(task.updatedAt);
  if (Number.isNaN(updated)) return false;
  return now.getTime() - updated > STUCK_THRESHOLD_MS;
}

function relative(iso: string, now: Date): string {
  const ts = Date.parse(iso);
  if (Number.isNaN(ts)) return "unknown";
  const delta = now.getTime() - ts;
  if (delta < 0) return "just now";

  const sec = Math.floor(delta / 1000);
  if (sec < 60) return `${sec}s ago`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.floor(hr / 24);
  return `${day}d ago`;
}
