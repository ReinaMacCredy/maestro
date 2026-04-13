import type { Task, UpdateTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateUpdateInput } from "../domain/task-validators.js";
import { closeViaCloseCommand } from "../domain/task-errors.js";

export interface ClaimParams {
  readonly sessionId: string;
}

export interface UpdateTaskOpts {
  readonly patch: UpdateTaskInput;
  /**
   * When provided, applies the atomic `--claim` semantics on top of `patch`:
   * sets assignee to the session id AND status to in_progress in the same
   * write. The use case resolves the session at the command layer and hands
   * the final id in here so the use case stays deterministic.
   */
  readonly claim?: ClaimParams;
}

/**
 * Update a task. Rejects `--status closed` (route through closeTask instead)
 * so the close reason always gets captured.
 */
export async function updateTask(
  store: TaskStorePort,
  id: string,
  opts: UpdateTaskOpts,
): Promise<Task> {
  const patch = validateUpdateInput(opts.patch);

  if (patch.status === "closed") {
    throw closeViaCloseCommand();
  }

  // Merge --claim on top of the explicit patch.
  const effectivePatch: UpdateTaskInput = opts.claim
    ? { ...patch, assignee: opts.claim.sessionId, status: "in_progress" }
    : patch;

  return store.update(id, effectivePatch);
}
