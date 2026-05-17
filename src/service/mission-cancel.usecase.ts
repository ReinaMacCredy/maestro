import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { MissionStorePort } from "../repo/mission-store.port.js";
import { MissionNotFoundError } from "../repo/mission-store.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import type { Mission, MissionId } from "../types/mission.js";
import { isTerminalMissionState } from "../types/mission-state.js";
import { isTerminalTaskState } from "../types/task-state.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";
import { summarizeTasks } from "./task-summary.js";

export interface MissionCancelDeps {
  readonly missionStore: MissionStorePort;
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface MissionCancelInput {
  readonly mission_id: MissionId;
  readonly reason?: string;
}

export interface MissionCancelCascadeError {
  readonly taskId: string;
  readonly message: string;
}

export interface MissionCancelResult {
  readonly mission: Mission;
  readonly cancelledTaskIds: readonly string[];
  readonly cascadeErrors: readonly MissionCancelCascadeError[];
  readonly alreadyCancelled: boolean;
}

export class MissionCancelTerminalError extends Error {
  readonly missionId: MissionId;
  readonly state: string;
  constructor(missionId: MissionId, state: string) {
    super(
      `mission already in ${state}; cancel applies only to active or cancelled missions.`,
    );
    this.name = "MissionCancelTerminalError";
    this.missionId = missionId;
    this.state = state;
  }
}

// Cancel cascade per plan spec:
//   1. completed/failed -> error (different outcome shouldn't be re-stamped).
//   2. already cancelled -> idempotent success (re-asserting the dead state).
//   3. otherwise: abandon every active task, then transition mission to
//      cancelled. Best-effort: task abandon failures land in cascadeErrors
//      and the mission still cancels; operator handles stragglers via
//      `task abandon <id>`.
export async function missionCancel(
  deps: MissionCancelDeps,
  input: MissionCancelInput,
): Promise<MissionCancelResult> {
  const mission = await deps.missionStore.get(input.mission_id);
  if (!mission) throw new MissionNotFoundError(input.mission_id);
  if (mission.state === "cancelled") {
    return {
      mission,
      cancelledTaskIds: [],
      cascadeErrors: [],
      alreadyCancelled: true,
    };
  }
  if (isTerminalMissionState(mission.state)) {
    throw new MissionCancelTerminalError(mission.id, mission.state);
  }

  const tasks = await deps.taskStore.listByMissionId(mission.id);
  const cancelledTaskIds: string[] = [];
  const cascadeErrors: MissionCancelCascadeError[] = [];

  for (const task of tasks) {
    if (isTerminalTaskState(task.state)) continue;
    try {
      await deps.taskStore.update(task.id, { state: "abandoned" });
      await emitTransitionEvidence(
        {
          store: deps.evidenceStore,
          clock: deps.clock,
          idFactory: deps.idFactory,
        },
        {
          task_id: task.id,
          mission_id: mission.id,
          from_state: task.state,
          to_state: "abandoned",
          trigger_verb: "mission:cancel",
          reason: input.reason,
        },
      );
      cancelledTaskIds.push(task.id);
    } catch (err) {
      cascadeErrors.push({
        taskId: task.id,
        message: (err as Error).message,
      });
    }
  }

  const cancelled = new Set(cancelledTaskIds);
  const tasksAfter = tasks.map((t) => (cancelled.has(t.id) ? { ...t, state: "abandoned" as const } : t));
  const updated = await deps.missionStore.update(mission.id, {
    state: "cancelled",
    cancel_reason: input.reason,
  });
  await emitTransitionEvidence(
    {
      store: deps.evidenceStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    {
      mission_id: mission.id,
      from_state: mission.state,
      to_state: "cancelled",
      trigger_verb: "mission:cancel",
      trigger: "verb",
      cancelled_by: "user",
      reason: input.reason,
      task_summary: summarizeTasks(tasksAfter),
    },
  );

  return {
    mission: updated,
    cancelledTaskIds,
    cascadeErrors,
    alreadyCancelled: false,
  };
}
