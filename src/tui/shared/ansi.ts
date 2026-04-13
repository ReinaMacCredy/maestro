/**
 * ANSI escape sequence helpers shared by runtime code and integration tests.
 */

const ESC = "\x1b";
const CSI = `${ESC}[`;

export function moveTo(row: number, col: number): string {
  return `${CSI}${row + 1};${col + 1}H`;
}

export const hideCursor = `${CSI}?25l`;
export const showCursor = `${CSI}?25h`;

export const enterAltScreen = `${CSI}?1049h`;
export const exitAltScreen = `${CSI}?1049l`;
export const clearScreen = `${CSI}2J`;
export const clearLine = `${CSI}2K`;
export const enableMouse = `${CSI}?1000h${CSI}?1006h`;
export const disableMouse = `${CSI}?1000l${CSI}?1006l`;

export const reset = `${CSI}0m`;
export const bold = `${CSI}1m`;
export const dim = `${CSI}2m`;
export const resetIntensity = `${CSI}22m`;

export function setFg(color: number): string {
  if (color < 0) return `${CSI}39m`;
  return `${CSI}38;5;${color}m`;
}

export function setBg(color: number): string {
  if (color < 0) return `${CSI}49m`;
  return `${CSI}48;5;${color}m`;
}

export function style(fg: number, bg: number, isBold: boolean, isDim: boolean): string {
  let sequence = reset;
  if (fg >= 0) sequence += setFg(fg);
  if (bg >= 0) sequence += setBg(bg);
  if (isBold) sequence += bold;
  if (isDim) sequence += dim;
  return sequence;
}

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

export const BLOCK = {
  full: "\u2588",
  light: "\u2591",
  medium: "\u2592",
  dark: "\u2593",
} as const;
