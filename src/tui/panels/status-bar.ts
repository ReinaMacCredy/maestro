/**
 * Status bar panel -- status dot, label, progress bar, counts, elapsed.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { MISSION_STATUS_COLOR, PALETTE } from "../theme.js";
import { formatElapsed } from "../format.js";
import { renderProgressBar } from "../widgets/progress-bar.js";

export function renderStatusBar(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const w = rect.width;
  const y = rect.y;
  buf.fillRect(rect, " ");

  // Status dot + label
  const statusColor = MISSION_STATUS_COLOR[snap.effectiveStatus] ?? PALETTE.gray;
  buf.writeText(y, rect.x + 1, "*", { fg: statusColor, bold: true });
  const label = snap.effectiveStatus.toUpperCase();
  buf.writeText(y, rect.x + 3, label, { fg: statusColor, bold: true });

  // Progress bar
  const barStart = rect.x + 4 + label.length + 2;
  const barWidth = Math.min(20, w - (barStart - rect.x) - 30);
  if (barWidth > 4) {
    const pct = snap.featureProgress.total > 0
      ? snap.featureProgress.done / snap.featureProgress.total
      : 0;
    renderProgressBar(buf, y, barStart, {
      ratio: pct,
      width: barWidth,
      fg: PALETTE.green,
    });
  }

  // Counts
  const counts = `${snap.featureProgress.done}/${snap.featureProgress.total}`;
  const activeStr = snap.featureProgress.active > 0 ? ` [+${snap.featureProgress.active}]` : "";
  buf.writeText(y, barStart + barWidth + 1, counts + activeStr, { fg: PALETTE.brightWhite });

  // Elapsed on right
  const elapsed = `Elapsed: ${formatElapsed(snap.elapsedMs)}`;
  buf.writeText(y, rect.x + w - elapsed.length - 1, elapsed, { fg: PALETTE.gray });
}
