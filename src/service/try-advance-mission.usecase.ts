import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { MissionStorePort } from "../repo/mission-store.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import type { Mission } from "../types/mission.js";
import { isTerminalTaskState } from "../types/task-state.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";

export interface TryAdvanceMissionDeps {
  readonly missionStore: MissionStorePort;
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface TryAdvanceMissionInput {
  readonly mission_id?: string;
  readonly trigger_task_verb: "task:claim" | "task:ship" | "task:abandon";
}

// ADR-0011: missions auto-advance off the back of task transitions. This helper
// is idempotent (a no-op for missions already past the target state) so individual
// task verbs can call it without caring about ordering or replay safety.
export async function tryAdvanceMission(
  deps: TryAdvanceMissionDeps,
  input: TryAdvanceMissionInput,
): Promise<Mission | undefined> {
  if (!input.mission_id) return undefined;
  const mission = await deps.missionStore.get(input.mission_id);
  if (!mission) return undefined;

  if (input.trigger_task_verb === "task:claim") {
    if (mission.state !== "planned") return mission;
    return advance(deps, mission, "in-progress", "mission:auto-start");
  }

  if (mission.state !== "in-progress" && mission.state !== "planned") return mission;
  const children = await deps.taskStore.listByMissionId(mission.id);
  if (children.length === 0) return mission;
  const allTerminal = children.every((t) => isTerminalTaskState(t.state));
  if (!allTerminal) return mission;
  // If we're still at 'planned' (e.g. every claimed task abandoned before
  // anyone advanced the mission), pass through in-progress so the state
  // machine stays well-formed.
  if (mission.state === "planned") {
    const stepped = await advance(deps, mission, "in-progress", "mission:auto-start");
    return advance(deps, stepped, "completed", "mission:auto-complete");
  }
  return advance(deps, mission, "completed", "mission:auto-complete");
}

async function advance(
  deps: TryAdvanceMissionDeps,
  mission: Mission,
  to: "in-progress" | "completed",
  trigger_verb: string,
): Promise<Mission> {
  const updated = await deps.missionStore.update(mission.id, { state: to });
  await emitTransitionEvidence(
    {
      store: deps.evidenceStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    {
      mission_id: mission.id,
      from_state: mission.state,
      to_state: to,
      trigger_verb,
    },
  );
  return updated;
}
