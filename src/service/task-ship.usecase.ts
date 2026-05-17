import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { MissionStorePort } from "../repo/mission-store.port.js";
import type { ObservabilityPort } from "../repo/observability.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { TaskNotFoundError } from "../repo/task-store.port.js";
import { assertTaskTransition } from "../types/task-state.js";
import type { Task, TaskId } from "../types/task.js";
import { assertMissionActive } from "./assert-mission-active.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";
import { tryAdvanceMission } from "./try-advance-mission.usecase.js";

export interface TaskShipDeps {
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly missionStore?: MissionStorePort;
  readonly observabilityStore?: ObservabilityPort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface TaskShipInput {
  readonly id: TaskId;
  readonly pr_url?: string;
}

export async function taskShip(deps: TaskShipDeps, input: TaskShipInput): Promise<Task> {
  const existing = await deps.taskStore.get(input.id);
  if (!existing) throw new TaskNotFoundError(input.id);
  await assertMissionActive(deps.missionStore, existing.mission_id, "task:ship");
  assertTaskTransition(existing.state, "shipped");
  const merged_at = (deps.clock ?? (() => new Date()))().toISOString();
  const updated = await deps.taskStore.update(input.id, {
    state: "shipped",
    pr_url: input.pr_url,
    merged_at,
  });
  await emitTransitionEvidence(
    {
      store: deps.evidenceStore,
      observabilityStore: deps.observabilityStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    {
      task_id: existing.id,
      from_state: existing.state,
      to_state: "shipped",
      trigger_verb: "task:ship",
      verdict: "PASS",
    },
  );
  if (deps.missionStore) {
    await tryAdvanceMission(
      {
        missionStore: deps.missionStore,
        taskStore: deps.taskStore,
        evidenceStore: deps.evidenceStore,
        clock: deps.clock,
        idFactory: deps.idFactory,
      },
      { mission_id: updated.mission_id, trigger_task_verb: "task:ship" },
    );
  }
  return updated;
}
