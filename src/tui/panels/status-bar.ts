/**
 * Status bar panel -- ● RUNNING [====] 0/4 [+2]
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { MISSION_STATUS_COLOR, MISSION_STATUS_LABEL, PALETTE, DOT_FILLED } from "../theme.js";
import { BLOCK } from "../terminal/ansi.js";

export function renderStatusBar(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const w = rect.width;
  const y = rect.y;
  buf.fillRect(rect, " ");

  const statusColor = MISSION_STATUS_COLOR[snap.effectiveStatus] ?? PALETTE.gray;
  const label = MISSION_STATUS_LABEL[snap.effectiveStatus] ?? snap.effectiveStatus.toUpperCase();

  // ● STATUS
  buf.writeText(y, rect.x + 1, DOT_FILLED, { fg: statusColor, bold: true });
  const labelEnd = rect.x + 3 + label.length;
  buf.writeText(y, rect.x + 3, label, { fg: statusColor, bold: true });

  // Progress bar: wide green filled blocks
  const countsStr = `${snap.featureProgress.done}/${snap.featureProgress.total}`;
  const activeStr = snap.featureProgress.active > 0 ? ` [+${snap.featureProgress.active}]` : "";
  const rightText = countsStr + activeStr;
  const rightStart = rect.x + w - rightText.length - 1;

  const barStart = labelEnd + 2;
  const barEnd = rightStart - 2;
  const barWidth = barEnd - barStart;

  if (barWidth > 4) {
    const pct = snap.featureProgress.total > 0
      ? snap.featureProgress.done / snap.featureProgress.total
      : 0;
    const filled = Math.round(pct * barWidth);

    for (let i = 0; i < barWidth; i++) {
      buf.set(y, barStart + i, i < filled ? BLOCK.full : BLOCK.dark, {
        fg: i < filled ? PALETTE.green : PALETTE.dimGray,
      });
    }
  }

  // Counts on right
  buf.writeText(y, rightStart, rightText, { fg: PALETTE.brightWhite, bold: true });
}
