import type { MissionControlMemorySnapshot } from "./types.js";

// Memory/graph projection was retired with the legacy memory and graph
// subsystems. Mission Control no longer surfaces a memory pane; this
// stub remains so existing call sites keep their typed shape.
export async function buildMissionControlMemorySnapshot(
  _deps: { cwd: string },
): Promise<MissionControlMemorySnapshot | null> {
  return null;
}
