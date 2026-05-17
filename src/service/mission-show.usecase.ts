import type { MissionStorePort } from "../repo/mission-store.port.js";
import { MissionNotFoundError } from "../repo/mission-store.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import type { Mission, MissionId } from "../types/mission.js";
import type { Task } from "../types/task.js";

export interface MissionShowDeps {
  readonly missionStore: MissionStorePort;
  readonly taskStore: TaskStorePort;
}

export interface MissionShowResult {
  readonly mission: Mission;
  readonly tasks: readonly Task[];
}

export async function missionShow(deps: MissionShowDeps, id: MissionId): Promise<MissionShowResult> {
  const mission = await deps.missionStore.get(id);
  if (!mission) throw new MissionNotFoundError(id);
  const tasks = await deps.taskStore.listByMissionId(id);
  return { mission, tasks };
}
