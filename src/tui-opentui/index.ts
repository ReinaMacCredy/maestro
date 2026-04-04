import { createLegacyMissionControlRenderer } from "../tui/legacy-renderer.js";
import type { MissionControlRenderer } from "../tui/mission-control-renderer.js";
import { runOpenTuiRenderCheck } from "./app/render-check.js";
import { renderOpenTuiPreviewFrame } from "./app/preview.js";

export { MissionControlApp, type MissionControlAppProps } from "./app/mission-control-app.js";

export function createOpenTuiMissionControlRenderer(): MissionControlRenderer {
  const legacy = createLegacyMissionControlRenderer();
  return {
    renderDashboard: legacy.renderDashboard,
    renderPreviewFrame: renderOpenTuiPreviewFrame,
    runRenderCheck: runOpenTuiRenderCheck,
  };
}
