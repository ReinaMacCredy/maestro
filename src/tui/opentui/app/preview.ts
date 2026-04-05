import { buildPreviewState } from "../../app/preview-state.js";
import type { PreviewFrameOptions } from "../../app/preview-contract.js";
import { capturedFrameToAnsi } from "../ansi.js";
import { captureMissionControlRender } from "../testing/frame-capture.js";

export async function renderOpenTuiPreviewFrame(opts: PreviewFrameOptions): Promise<string> {
  const width = opts.width ?? Math.min(process.stdout.columns || 120, 200);
  const minHeight = Math.max(opts.snapshot.features.length * 2 + 24, 36);
  const height = opts.height ?? Math.max(process.stdout.rows || 0, minHeight);
  const state = buildPreviewState(opts);
  const format = opts.format ?? (process.stdout.isTTY ? "ansi" : "plain");
  const render = await captureMissionControlRender({
    snapshot: opts.snapshot,
    state,
    width,
    height,
  });
  return format === "ansi" ? capturedFrameToAnsi(render.spans) : render.charFrame;
}
