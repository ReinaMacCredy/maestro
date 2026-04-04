import type { InteractiveOptions } from "./app/app.js";
import type { RenderCheckResult } from "./app/render-check.js";
import type { PreviewFrameOptions } from "./app/render.js";
import type { MissionControlSnapshot } from "./state/types.js";

export interface MissionControlRenderCheckOptions {
  readonly width?: number;
  readonly height?: number;
}

export interface MissionControlRenderer {
  renderDashboard(opts: InteractiveOptions): Promise<void>;
  renderPreviewFrame(opts: PreviewFrameOptions): Promise<string>;
  runRenderCheck(
    snapshot: MissionControlSnapshot,
    opts?: MissionControlRenderCheckOptions,
  ): Promise<RenderCheckResult>;
}
