import type { PreviewStateOptions } from "./preview-state.js";

export interface PreviewFrameOptions extends PreviewStateOptions {
  readonly width?: number;
  readonly height?: number;
  readonly format?: "plain" | "ansi";
}
