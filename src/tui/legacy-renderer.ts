import { renderDashboard } from "./app/app.js";
import { runRenderCheck } from "./app/render-check.js";
import { renderPreviewFrame } from "./app/render.js";
import type { MissionControlRenderer } from "./mission-control-renderer.js";

export function createLegacyMissionControlRenderer(): MissionControlRenderer {
  return {
    renderDashboard,
    renderPreviewFrame: async (opts) => renderPreviewFrame(opts),
    runRenderCheck: async (snapshot, opts) => runRenderCheck(snapshot, opts),
  };
}
