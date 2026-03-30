/**
 * ANSI escape sequence helpers (CSI primitives)
 * Zero dependencies -- pure string builders for terminal control.
 */

const ESC = "\x1b";
const CSI = `${ESC}[`;

// ── Cursor ──────────────────────────────────────────

export function moveTo(row: number, col: number): string {
  return `${CSI}${row + 1};${col + 1}H`;
}

export const hideCursor = `${CSI}?25l`;
export const showCursor = `${CSI}?25h`;

// ── Screen ──────────────────────────────────────────

export const enterAltScreen = `${CSI}?1049h`;
export const exitAltScreen = `${CSI}?1049l`;
export const clearScreen = `${CSI}2J`;
export const clearLine = `${CSI}2K`;

// ── Style ───────────────────────────────────────────

export const reset = `${CSI}0m`;
export const bold = `${CSI}1m`;
export const dim = `${CSI}2m`;
export const resetIntensity = `${CSI}22m`;

/** Set foreground to 256-color index. -1 = default. */
export function setFg(color: number): string {
  if (color < 0) return `${CSI}39m`;
  return `${CSI}38;5;${color}m`;
}

/** Set background to 256-color index. -1 = default. */
export function setBg(color: number): string {
  if (color < 0) return `${CSI}49m`;
  return `${CSI}48;5;${color}m`;
}

/** Build a full style string from Cell-like attributes. */
export function style(fg: number, bg: number, isBold: boolean, isDim: boolean): string {
  let s = reset;
  if (fg >= 0) s += setFg(fg);
  if (bg >= 0) s += setBg(bg);
  if (isBold) s += bold;
  if (isDim) s += dim;
  return s;
}

// ── Box Drawing ─────────────────────────────────────

export const BOX = {
  topLeft: "\u250c",
  topRight: "\u2510",
  bottomLeft: "\u2514",
  bottomRight: "\u2518",
  horizontal: "\u2500",
  vertical: "\u2502",
  teeDown: "\u252c",
  teeUp: "\u2534",
  teeRight: "\u251c",
  teeLeft: "\u2524",
  cross: "\u253c",
} as const;

// ── Block Characters ────────────────────────────────

export const BLOCK = {
  full: "\u2588",
  light: "\u2591",
  medium: "\u2592",
  dark: "\u2593",
} as const;
