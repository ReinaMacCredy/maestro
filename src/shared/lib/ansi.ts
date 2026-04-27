/**
 * Tiny zero-dependency ANSI helper for CLI text output.
 *
 * Color is suppressed when:
 * - the `NO_COLOR` env var is set (https://no-color.org/), or
 * - `process.stdout.isTTY` is false (piped, redirected, or non-terminal).
 *
 * Callers pass a color name to `colorize`; when color is disabled the original
 * string is returned unchanged so non-terminal callers see plain output.
 */

export type AnsiColor = "cyan" | "green" | "red" | "yellow" | "dim";

const ESC = "\x1b";
const RESET = `${ESC}[0m`;

const CODES: Readonly<Record<AnsiColor, string>> = {
  cyan: `${ESC}[36m`,
  green: `${ESC}[32m`,
  red: `${ESC}[31m`,
  yellow: `${ESC}[33m`,
  dim: `${ESC}[2m`,
};

export function isColorEnabled(): boolean {
  if (process.env.NO_COLOR !== undefined && process.env.NO_COLOR !== "") {
    return false;
  }
  if (process.env.FORCE_COLOR !== undefined && process.env.FORCE_COLOR !== "") {
    return true;
  }
  return Boolean(process.stdout.isTTY);
}

export function colorize(text: string, color: AnsiColor, enabled: boolean): string {
  if (!enabled) return text;
  return `${CODES[color]}${text}${RESET}`;
}
