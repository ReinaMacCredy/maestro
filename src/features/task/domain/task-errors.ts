import { MaestroError } from "@/shared/errors.js";

// ============================
// Task Error Factories
// ============================

export function taskNotFound(id: string): MaestroError {
  return new MaestroError(`Task ${id} not found`, [
    "List tasks: maestro task list",
    `Check that task ID '${id}' is correct`,
  ]);
}

export function unknownBlocker(id: string, missing: readonly string[]): MaestroError {
  return new MaestroError(
    `Task ${id} references unknown blocker task(s): ${missing.join(", ")}`,
    [
      "Create the referenced blocker task(s) first",
      "Or remove the unknown IDs from --blocked-by / task block",
      "List existing tasks: maestro task list",
    ],
  );
}

export function taskSelfBlock(id: string): MaestroError {
  return new MaestroError(
    `Task ${id} cannot block itself`,
    [
      "Remove the task id from the blocker edge",
      "Use a different task id with task block",
    ],
  );
}

export function taskBlockCycle(id: string, chain: readonly string[]): MaestroError {
  return new MaestroError(
    `Task blocker cycle detected for ${id}: ${chain.join(" -> ")}`,
    [
      "A task cannot block a chain that leads back to itself",
      "Remove one of the blocker edges before retrying",
    ],
  );
}

export function cyclicParent(id: string, chain: readonly string[]): MaestroError {
  return new MaestroError(
    `Cyclic parent chain detected for ${id}: ${chain.join(" -> ")}`,
    [
      "A task cannot be its own ancestor through the parent chain",
      "Choose a different parent or move the task to the root",
    ],
  );
}

export function invalidTaskField(field: string, reason: string): MaestroError {
  return new MaestroError(`Invalid task ${field}: ${reason}`);
}

export function taskUpdateOwnershipViaClaim(): MaestroError {
  return new MaestroError(
    "Task ownership must be managed via dedicated claim commands",
    [
      "Use 'maestro task claim <id>' to take ownership",
      "Use 'maestro task unclaim <id>' to release ownership",
    ],
  );
}

export function taskUpdateClaimViaDedicatedCommand(): MaestroError {
  return new MaestroError(
    "Task claiming moved to dedicated commands",
    [
      "Use 'maestro task claim <id>' instead of 'task update --claim'",
      "Use 'maestro task unclaim <id>' to release ownership",
    ],
  );
}

export function taskDependencyCommandsRenamed(): MaestroError {
  return new MaestroError(
    "Task dependency commands were replaced by blocker commands",
    [
      "Use 'maestro task block <blockerId> <blockedId...>'",
      "Use 'maestro task unblock <blockerId> <blockedId...>'",
      "Use '--blocked-by <ids>' when creating a task",
    ],
  );
}

export function taskCompletedViaUpdateStatus(): MaestroError {
  return new MaestroError(
    "Task completion moved to update status",
    [
      "Use 'maestro task update <id> --status completed --reason \"...\"'",
      "Completed tasks keep their reason for task-ready hints",
    ],
  );
}

export function taskReasonRequiresCompletedStatus(): MaestroError {
  return new MaestroError(
    "Task completion reason requires status 'completed'",
    [
      "Use 'maestro task update <id> --status completed --reason \"...\"'",
      "Or omit --reason when updating a task to a non-completed status",
    ],
  );
}

export function taskAlreadyCompleted(id: string): MaestroError {
  return new MaestroError(
    `Task ${id} is already completed`,
    [
      "Use 'maestro task show <id>' to inspect the existing completion reason",
      "Completed tasks are immutable; create a follow-up task instead",
    ],
  );
}

export function taskStatusRequiresClaim(status: "in_progress"): MaestroError {
  return new MaestroError(
    `Status '${status}' requires task ownership`,
    [
      "Use 'maestro task claim <id>' before moving work into progress",
      "Or leave the task in 'pending' until an owner is assigned",
    ],
  );
}

export function claimedTaskCannotBeReopened(id: string): MaestroError {
  return new MaestroError(
    `Task ${id} cannot move to 'pending' while still claimed`,
    [
      "Use 'maestro task unclaim <id>' to release ownership first",
      "Or keep the task in progress while the owner still holds it",
    ],
  );
}

export function taskAlreadyClaimed(id: string, assignee: string): MaestroError {
  return new MaestroError(
    `Task ${id} is already claimed by ${assignee}`,
    [
      "Use 'maestro task show <id>' to inspect current ownership",
      "Use 'maestro task claim <id> --force' for an explicit takeover",
      "Pass '--session <id>' when forcing takeover outside an agent session",
    ],
  );
}

export function taskNotClaimed(id: string): MaestroError {
  return new MaestroError(
    `Task ${id} is not currently claimed`,
    [
      "Use 'maestro task claim <id>' to take ownership first",
    ],
  );
}

export function taskClaimOwnedByDifferentSession(id: string, assignee: string): MaestroError {
  return new MaestroError(
    `Task ${id} is claimed by ${assignee}`,
    [
      "Use 'maestro task unclaim <id> --force' for an explicit admin release",
      "Pass '--session <id>' when forcing release outside an agent session",
      "Or ask the current owner to release the task",
    ],
  );
}

export function taskBlockedByOpenTasks(id: string, blockers: readonly string[]): MaestroError {
  return new MaestroError(
    `Task ${id} is blocked by unresolved task(s): ${blockers.join(", ")}`,
    [
      "Resolve or complete the blocker tasks before starting or completing this task",
      "Use 'maestro task show <id>' to inspect blocker relationships",
      "Blocked tasks cannot be claimed or completed while blockers remain unresolved",
    ],
  );
}

export function taskClaimBusySession(sessionId: string, taskIds: readonly string[]): MaestroError {
  return new MaestroError(
    `Session ${sessionId} already owns unresolved task(s): ${taskIds.join(", ")}`,
    [
      "Finish or unclaim the existing task before claiming another one",
      "Retry without the busy-check path if multiple concurrent claims are intentional",
    ],
  );
}

export function parentDepthExceeded(id: string, depth: number): MaestroError {
  return new MaestroError(
    `Task ${id} parent chain exceeds depth ${depth}`,
    [
      "This usually indicates a malformed parent chain",
      "Run 'maestro task show <id>' on each ancestor to inspect the chain",
    ],
  );
}
