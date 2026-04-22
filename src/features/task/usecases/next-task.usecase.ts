import type { Task, ReadyTasksFilters } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { MaestroError } from "@/shared/errors.js";
import { isTaskAlreadyClaimedError } from "../domain/task-errors.js";
import { claimTask } from "./claim-task.usecase.js";
import { listTasks } from "./list-tasks.usecase.js";
import { readyTaskPage } from "./ready-tasks.usecase.js";

export type NextTaskReason = "nothing pending" | "all blocked" | "race";

export interface NextTaskInput {
  readonly sessionId: string;
  readonly force?: boolean;
  readonly filters?: ReadyTasksFilters;
}

export interface NextTaskResult {
  readonly task?: Task;
  readonly reason?: NextTaskReason;
}

const CANDIDATE_LIMIT = 8;

export async function nextTask(
  store: TaskStorePort,
  input: NextTaskInput,
): Promise<NextTaskResult> {
  if (!input.force) {
    const owned = await listTasks(store, { assignee: input.sessionId });
    const held = owned.filter((task) => task.status !== "completed");
    if (held.length > 0) {
      const ids = held.map((task) => task.id);
      const first = ids[0]!;
      throw new MaestroError(
        `You already hold ${first}; update or unclaim before pulling another`,
        [
          `Held task ids: ${ids.join(", ")}`,
          `Run 'maestro task update ${first} --status completed' when finished`,
          `Run 'maestro task unclaim ${first}' to release without completing`,
          `Pass '--force' to pull another while holding ${first}`,
        ],
      );
    }
  }

  const page = await readyTaskPage(store, {
    ...(input.filters ?? {}),
    limit: input.filters?.limit ?? CANDIDATE_LIMIT,
  });

  if (page.totalReady === 0) {
    return { reason: page.totalPending === 0 ? "nothing pending" : "all blocked" };
  }

  for (const candidate of page.items) {
    try {
      const claimed = await claimTask(store, candidate.id, { sessionId: input.sessionId });
      return { task: claimed };
    } catch (error) {
      if (isTaskAlreadyClaimedError(error)) {
        continue;
      }
      throw error;
    }
  }

  return { reason: "race" };
}
