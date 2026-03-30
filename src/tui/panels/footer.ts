/**
 * Footer panel -- key binding hints with bold key letters.
 * "F Features  W Workers  M Models  P Pause  D Mission Dir  Ctrl+T Back To Orchestrator"
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

  const hints: FooterHint[] = [
    { key: "F", label: "Features" },
    { key: "W", label: "Workers" },
    { key: "M", label: "Models" },
  ];

  if (snap.canPause) hints.push({ key: "P", label: "Pause" });
  if (snap.canResume) hints.push({ key: "P", label: "Resume" });

  hints.push({ key: "D", label: "Mission Dir" });
  hints.push({ key: "Ctrl+T", label: "Back To Orchestrator" });

  let col = rect.x + 1;
  for (const hint of hints) {
    if (col + hint.key.length + hint.label.length + 4 > rect.x + w) break;

    // Bold key
    buf.writeText(y, col, hint.key, { fg: PALETTE.brightWhite, bg: PALETTE.headerBg, bold: true });
    col += hint.key.length + 1;
    // Label
    buf.writeText(y, col, hint.label, { fg: PALETTE.gray, bg: PALETTE.headerBg });
    col += hint.label.length + 4;
  }
}
