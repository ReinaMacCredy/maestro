import { countMetCriteria, isActiveContract } from "./contract/contract-state.js";
import type { Contract } from "./contract/contract-types.js";
import type { Task } from "./task-types.js";
import { indexTasksById } from "./task-types.js";
import { hasUnresolvedBlockers } from "./task-state.js";
import { formatRelativeAge } from "@/shared/version-format.js";

export const STUCK_THRESHOLD_MS = 4 * 60 * 60 * 1000;

const READY_LIMIT = 5;
const DESCRIPTION_TRUNCATE = 300;

export interface BuildNowMdInput {
  readonly tasks: readonly Task[];
  readonly now: Date;
  readonly contracts?: ReadonlyMap<string, Contract>;
}

export function buildNowMd({ tasks, now, contracts = new Map() }: BuildNowMdInput): string {
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

  const stuck = inProgress.filter((task) => isStuckTask(task, now));

  const lines: string[] = [];
  lines.push("# NOW");
  lines.push(`Updated: ${updated}`);
  lines.push("");

  lines.push(`## In progress (${inProgress.length})`);
  if (inProgress.length === 0) {
    lines.push("None.");
  } else {
    for (const task of inProgress) {
      lines.push(...renderTask(task, now, byId, contracts, { includeOwner: true, includeContract: true }));
    }
  }
  lines.push("");

  lines.push(`## Ready to pick up (${ready.length})`);
  if (ready.length === 0) {
    lines.push("None.");
  } else {
    for (const task of ready) {
      lines.push(...renderTask(task, now, byId, contracts, { includeOwner: false, includeContract: false }));
    }
  }
  lines.push("");

  lines.push(`## Stuck (${stuck.length})`);
  if (stuck.length === 0) {
    lines.push("None.");
  } else {
    for (const task of stuck) {
      lines.push(...renderTask(task, now, byId, contracts, { includeOwner: true, includeContract: false }));
    }
  }

  return lines.join("\n") + "\n";
}

export function isStuckTask(task: Task, now: Date, thresholdMs: number = STUCK_THRESHOLD_MS): boolean {
  const last = lastActivityMs(task);
  if (last === undefined) return false;
  return now.getTime() - last > thresholdMs;
}

function lastActivityMs(task: Task): number | undefined {
  const source = task.lastActivityAt ?? task.updatedAt;
  const parsed = Date.parse(source);
  return Number.isNaN(parsed) ? undefined : parsed;
}

function renderTask(
  task: Task,
  now: Date,
  byId: ReadonlyMap<string, Task>,
  contracts: ReadonlyMap<string, Contract>,
  opts: { includeOwner: boolean; includeContract: boolean },
): readonly string[] {
  const out: string[] = [];
  out.push(`### ${task.id} . ${task.title}`);

  if (opts.includeOwner && task.assignee) {
    const claimed = task.claimedAt ? formatRelativeAge(task.claimedAt, now) : "unknown";
    const activity = formatRelativeAge(task.lastActivityAt ?? task.updatedAt, now);
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

  const contract = opts.includeContract ? resolveActiveContract(task, contracts) : undefined;
  if (contract) {
    const inherited = latestInheritanceSource(contract);
    out.push(
      `Contract: ${contract.id} (${contract.status}${inherited ? `, inherited from ${inherited}` : ""}, ${countMetCriteria(contract.doneWhen)}/${contract.doneWhen.length} done-when met, scope: ${summarizeScope(contract)})`,
    );
  }

  out.push("");
  return out;
}

function resolveActiveContract(task: Task, contracts: ReadonlyMap<string, Contract>): Contract | undefined {
  if (!task.contractId) {
    return undefined;
  }
  const contract = contracts.get(task.contractId);
  if (!contract || !isActiveContract(contract)) {
    return undefined;
  }
  return contract;
}

function summarizeScope(contract: Contract): string {
  const first = contract.scope.filesExpected[0];
  if (!first) {
    return "(none)";
  }
  const extra = contract.scope.filesExpected.length - 1;
  return extra > 0 ? `${first} +${extra} more` : first;
}

function latestInheritanceSource(contract: Contract): string | undefined {
  const latest = contract.ownershipHistory?.at(-1);
  if (!latest || latest.to !== contract.lockedBy) {
    return undefined;
  }
  return latest.from;
}

function byPriorityThenCreated(a: Task, b: Task): number {
  if (a.priority !== b.priority) return a.priority - b.priority;
  return a.createdAt.localeCompare(b.createdAt);
}
