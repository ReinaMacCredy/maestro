/**
 * Reusable progress bar widget.
 * Renders: [====>     ] 45%
 */
import { Buffer } from "../terminal/buffer.js";
import { PALETTE } from "../theme.js";

export interface ProgressBarOptions {
  /** 0.0 to 1.0 */
  ratio: number;
  width: number;
  fg?: number;
  bg?: number;
}

/**
 * Render a progress bar into the buffer at (row, col).
 * Returns the number of columns consumed.
 */
export function renderProgressBar(
  buf: Buffer,
  row: number,
  col: number,
  opts: ProgressBarOptions,
): number {
  const { ratio, width, fg = PALETTE.green, bg } = opts;
  if (width < 4) return 0;

  const innerWidth = width - 2; // space for [ and ]
  const pct = Math.max(0, Math.min(1, ratio));
  const filled = Math.round(pct * innerWidth);

  let bar = "[";
  for (let i = 0; i < innerWidth; i++) {
    bar += i < filled ? "=" : " ";
  }
  bar += "]";

  buf.writeText(row, col, bar, { fg, bg });
  return width;
}
