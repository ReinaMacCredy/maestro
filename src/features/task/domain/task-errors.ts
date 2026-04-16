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

export function unknownDependency(id: string, missing: readonly string[]): MaestroError {
  return new MaestroError(
    `Task ${id} references unknown task(s): ${missing.join(", ")}`,
    [
      "Create the referenced task(s) first",
      "Or remove the unknown IDs from --depends-on",
      "List existing tasks: maestro task list",
    ],
  );
}

export function taskSelfDependency(id: string): MaestroError {
  return new MaestroError(
    `Task ${id} cannot depend on itself`,
    [
      "Remove the task id from the dependency list",
      "Use other task ids in --depends-on or task deps add",
    ],
  );
}

export function taskDependencyCycle(id: string, chain: readonly string[]): MaestroError {
  return new MaestroError(
    `Dependency cycle detected for ${id}: ${chain.join(" -> ")}`,
    [
      "A task cannot depend on a chain that leads back to itself",
      "Remove one of the dependency edges before retrying",
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

export function closeViaCloseCommand(): MaestroError {
  return new MaestroError(
    "Cannot set status to 'closed' via update",
    [
      "Use 'maestro task close <id> --reason \"...\"' to close a task",
      "The reason field is captured on close and shows up in the audit trail",
    ],
  );
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

export function taskAlreadyClosed(id: string): MaestroError {
  return new MaestroError(
    `Task ${id} is already closed`,
    [
      "Use 'maestro task show <id>' to inspect the existing close reason",
      "Closed tasks are immutable; create a follow-up task instead of re-closing",
    ],
  );
}

export function taskAlreadyClaimed(id: string, assignee: string): MaestroError {
  return new MaestroError(
    `Task ${id} is already claimed by ${assignee}`,
    [
      "Use 'maestro task show <id>' to inspect current ownership",
      "Use 'maestro task claim <id> --force' for an explicit takeover",
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
      "Or ask the current owner to release the task",
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
