/**
 * Footer panel -- key binding hints with bold key letters.
 * Mission mode: "F Features  H Handoff  C Config  P Processes  Ctrl+T Exit"
 * Home mode: "F Overview  H Handoff  C Config  P Processes  Ctrl+T Exit"
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { PALETTE } from "../theme.js";

interface FooterHint {
  key: string;
  label: string;
}

export function renderFooter(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const w = rect.width;
  const y = rect.y;
  buf.fillRect(rect, " ", { bg: PALETTE.headerBg });

  const leftHints = snap.mode === "home"
    ? [
      { key: "F", label: "Overview" },
      { key: "H", label: "Handoff" },
      { key: "C", label: "Config" },
      { key: "P", label: "Processes" },
    ]
    : [
      { key: "F", label: "Features" },
      { key: "H", label: "Handoff" },
      { key: "C", label: "Config" },
      { key: "P", label: "Processes" },
    ];

  const exitHint = { key: "Ctrl+T", label: "Exit" };
  const exitWidth = exitHint.key.length + exitHint.label.length + 2;
  const exitCol = rect.x + w - exitWidth - 1;

  renderHint(buf, y, exitCol, exitHint);

  let col = rect.x + 1;
  for (const hint of leftHints) {
    const hintWidth = hint.key.length + hint.label.length + 4;
    if (col + hintWidth >= exitCol - 1) break;
    renderHint(buf, y, col, hint);
    col += hintWidth;
  }
}

function renderHint(buf: Buffer, y: number, col: number, hint: FooterHint): void {
  buf.writeText(y, col, hint.key, { fg: PALETTE.brightWhite, bg: PALETTE.headerBg, bold: true });
  buf.writeText(y, col + hint.key.length + 1, hint.label, { fg: PALETTE.gray, bg: PALETTE.headerBg });
}
