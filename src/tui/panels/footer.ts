/**
 * Footer panel -- key binding hints with bold key letters.
 * Mission mode: "F Features  H Handoff  C Config  P Processes  Ctrl+P Commands  Ctrl+T Exit"
 * Home mode: "F Overview  H Handoff  C Config  P Processes  Ctrl+P Commands  Ctrl+T Exit"
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { getMissionControlCommandSpecs } from "../mission-control-commands.js";
import { PALETTE } from "../theme.js";

interface FooterHint {
  key: string;
  label: string;
}

export function renderFooter(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const w = rect.width;
  const y = rect.y;
  buf.fillRect(rect, " ", { bg: PALETTE.headerBg });

  const commands = getMissionControlCommandSpecs(snap.mode);
  const leftHints = commands
    .filter((command) => command.key.length === 1)
    .map((command) => ({ key: command.key, label: command.label }));
  const commandsHint = { key: "Ctrl+P", label: "Commands" };
  const exitCommand = commands.find((command) => command.id === "exit");
  const exitHint = {
    key: exitCommand?.key ?? "Ctrl+T",
    label: exitCommand?.label ?? "Exit",
  };
  const exitWidth = measureHint(exitHint);
  const commandsWidth = measureHint(commandsHint);
  const exitCol = rect.x + w - exitWidth - 1;
  const commandsCol = exitCol - commandsWidth - 3;

  if (commandsCol > rect.x + 1) {
    renderHint(buf, y, commandsCol, commandsHint);
  }
  renderHint(buf, y, exitCol, exitHint);

  let col = rect.x + 1;
  for (const hint of leftHints) {
    const hintWidth = measureHint(hint) + 2;
    const rightBoundary = commandsCol > rect.x + 1 ? commandsCol - 2 : exitCol - 2;
    if (col + hintWidth >= rightBoundary) break;
    renderHint(buf, y, col, hint);
    col += hintWidth;
  }
}

function renderHint(buf: Buffer, y: number, col: number, hint: FooterHint): void {
  buf.writeText(y, col, hint.key, { fg: PALETTE.brightWhite, bg: PALETTE.headerBg, bold: true });
  buf.writeText(y, col + hint.key.length + 1, hint.label, { fg: PALETTE.gray, bg: PALETTE.headerBg });
}

function measureHint(hint: FooterHint): number {
  return hint.key.length + hint.label.length + 1;
}
