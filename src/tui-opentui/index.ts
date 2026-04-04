import { createLegacyMissionControlRenderer } from "../tui/legacy-renderer.js";
import type { MissionControlRenderer } from "../tui/mission-control-renderer.js";

export { MissionControlApp, type MissionControlAppProps } from "./app/mission-control-app.js";

export function createOpenTuiMissionControlRenderer(): MissionControlRenderer {
  return createLegacyMissionControlRenderer();
}
