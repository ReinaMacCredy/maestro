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

export function parentDepthExceeded(id: string, depth: number): MaestroError {
  return new MaestroError(
    `Task ${id} parent chain exceeds depth ${depth}`,
    [
      "This usually indicates a malformed parent chain",
      "Run 'maestro task show <id>' on each ancestor to inspect the chain",
    ],
  );
}
