import type { MissionStorePort } from "../repo/mission-store.port.js";
import { isTerminalMissionState, type MissionState } from "../types/mission-state.js";

export class MissionTerminalGuardError extends Error {
  readonly missionId: string;
  readonly missionState: MissionState;
  readonly verb: string;
  constructor(missionId: string, missionState: MissionState, verb: string) {
    super(
      `cannot ${verb}: parent mission ${missionId} is ${missionState} (terminal). ` +
        `Move the task to a different mission or accept the orphan via \`task abandon\`.`,
    );
    this.name = "MissionTerminalGuardError";
    this.missionId = missionId;
    this.missionState = missionState;
    this.verb = verb;
  }
}

// Guard for task verbs that meaningfully advance task state under a parent
// mission (claim, ship). Refuses to operate when the parent mission has
// already terminated so orphans surfaced after `mission cancel` cascade
// failures can't be silently shipped/claimed under a dead mission.
//
// Lenient when the store or mission_id is absent, and when the referenced
// mission can't be found — matches tryAdvanceMission's tolerance for stale
// pointers (data-integrity bugs are a different surface).
export async function assertMissionActive(
  store: MissionStorePort | undefined,
  missionId: string | undefined,
  verb: string,
): Promise<void> {
  if (!store || !missionId) return;
  const mission = await store.get(missionId);
  if (!mission) return;
  if (isTerminalMissionState(mission.state)) {
    throw new MissionTerminalGuardError(missionId, mission.state, verb);
  }
}
