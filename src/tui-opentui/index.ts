import { createLegacyMissionControlRenderer } from "../tui/legacy-renderer.js";
import type { MissionControlRenderer } from "../tui/mission-control-renderer.js";
import { renderOpenTuiDashboard } from "./app/interactive.js";
import { runOpenTuiRenderCheck } from "./app/render-check.js";
import { renderOpenTuiPreviewFrame } from "./app/preview.js";

export { MissionControlApp, type MissionControlAppProps } from "./app/mission-control-app.js";

export function createOpenTuiMissionControlRenderer(): MissionControlRenderer {
  return {
    renderDashboard: renderOpenTuiDashboard,
    renderPreviewFrame: renderOpenTuiPreviewFrame,
    runRenderCheck: runOpenTuiRenderCheck,
  };
}
