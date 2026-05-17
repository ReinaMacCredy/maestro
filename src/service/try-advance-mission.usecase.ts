import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { MissionStorePort } from "../repo/mission-store.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import type { Mission } from "../types/mission.js";
import { isTerminalMissionState, type MissionState } from "../types/mission-state.js";
import { isTerminalTaskState } from "../types/task-state.js";
import type { Task } from "../types/task.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";
import { summarizeTasks } from "./task-summary.js";

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

const FIXED_POINT_CAP = 10;

export type MissionRollupRule =
  | "auto-start"
  | "auto-pause"
  | "auto-resume"
  | "complete-or-fail";

interface NextStep {
  readonly to: MissionState;
  readonly rule: MissionRollupRule;
  readonly trigger_verb: string;
}

// ADR-0011: missions auto-advance off the back of task transitions. Idempotent
// so individual task verbs can call it without caring about ordering or
// replay safety. Fixed-point loop re-applies the rules so a single task event
// can carry a mission across multiple states (e.g. planned -> in-progress ->
// completed when the last task ships).
export async function tryAdvanceMission(
  deps: TryAdvanceMissionDeps,
  input: TryAdvanceMissionInput,
): Promise<Mission | undefined> {
  if (!input.mission_id) return undefined;
  let mission = await deps.missionStore.get(input.mission_id);
  if (!mission) return undefined;
  if (isTerminalMissionState(mission.state)) return mission;

  // Tasks don't change inside the loop (only mission state does), so read once.
  const tasks = await deps.taskStore.listByMissionId(mission.id);

  for (let i = 0; i < FIXED_POINT_CAP; i += 1) {
    if (isTerminalMissionState(mission.state)) return mission;
    const next = computeNext(mission.state, tasks);
    if (!next) return mission;
    mission = await advanceRollup(deps, mission, next, tasks);
  }
  return mission;
}

function computeNext(state: MissionState, tasks: readonly Task[]): NextStep | undefined {
  if (tasks.length === 0) return undefined;

  if (state === "planned") {
    if (tasks.some((t) => t.state !== "draft")) {
      return {
        to: "in-progress",
        rule: "auto-start",
        trigger_verb: "mission:auto-start",
      };
    }
    return undefined;
  }

  const allTerminal = tasks.every((t) => isTerminalTaskState(t.state));
  if (allTerminal) {
    const allShipped = tasks.every((t) => t.state === "shipped");
    return allShipped
      ? {
          to: "completed",
          rule: "complete-or-fail",
          trigger_verb: "mission:auto-complete",
        }
      : {
          to: "failed",
          rule: "complete-or-fail",
          trigger_verb: "mission:auto-fail",
        };
  }

  const nonTerminal = tasks.filter((t) => !isTerminalTaskState(t.state));
  if (state === "in-progress") {
    if (nonTerminal.length > 0 && nonTerminal.every((t) => t.state === "blocked")) {
      return {
        to: "paused",
        rule: "auto-pause",
        trigger_verb: "mission:auto-pause",
      };
    }
    return undefined;
  }
  if (state === "paused") {
    if (nonTerminal.some((t) => t.state !== "blocked")) {
      return {
        to: "in-progress",
        rule: "auto-resume",
        trigger_verb: "mission:auto-resume",
      };
    }
    return undefined;
  }
  return undefined;
}

async function advanceRollup(
  deps: TryAdvanceMissionDeps,
  mission: Mission,
  next: NextStep,
  tasks: readonly Task[],
): Promise<Mission> {
  const updated = await deps.missionStore.update(mission.id, { state: next.to });
  await emitTransitionEvidence(
    {
      store: deps.evidenceStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    {
      mission_id: mission.id,
      from_state: mission.state,
      to_state: next.to,
      trigger_verb: next.trigger_verb,
      trigger: "rollup",
      rule: next.rule,
      task_summary: summarizeTasks(tasks),
    },
  );
  return updated;
}

