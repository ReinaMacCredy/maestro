/**
 * Status bar panel -- ● RUNNING [green][dark] N/M [+K]
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

  if (snap.mode === "home" && snap.home) {
    const hasFail = snap.home.checks.some((check) => check.status === "fail");
    const hasWarn = snap.home.checks.some((check) => check.status === "warn");
    const statusColor = hasFail ? PALETTE.red : hasWarn ? PALETTE.yellow : PALETTE.blue;
    const summary = [
      `${snap.home.checks.length} checks`,
      `${snap.home.actions.length} next`,
      snap.home.pendingHandoffs.length > 0 ? `${snap.home.pendingHandoffs.length} handoffs` : undefined,
    ].filter(Boolean).join("  ·  ");

    buf.writeText(y, rect.x + 1, DOT_FILLED, { fg: statusColor, bold: true });
    buf.writeText(y, rect.x + 3, "HOME", { fg: statusColor, bold: true });
    buf.writeText(y, rect.x + 8, snap.home.headline, { fg: PALETTE.brightWhite, bold: true });

    const summaryX = rect.x + w - summary.length - 1;
    if (summaryX > rect.x + 18) {
      buf.writeText(y, summaryX, summary, { fg: PALETTE.gray });
    }
    return;
  }

  const statusColor = MISSION_STATUS_COLOR[snap.effectiveStatus] ?? PALETTE.gray;
  const label = MISSION_STATUS_LABEL[snap.effectiveStatus] ?? snap.effectiveStatus.toUpperCase();

  // ● STATUS
  buf.writeText(y, rect.x + 1, DOT_FILLED, { fg: statusColor, bold: true });
  const labelEnd = rect.x + 3 + label.length;
  buf.writeText(y, rect.x + 3, label, { fg: statusColor, bold: true });

  // Counts on right
  const countsStr = `${snap.featureProgress.done}/${snap.featureProgress.total}`;
  const activeStr = snap.featureProgress.active > 0 ? ` [+${snap.featureProgress.active}]` : "";
  const rightText = countsStr + activeStr;
  const rightStart = rect.x + w - rightText.length - 1;

  // Progress bar: green for (done + active), dark bg for unfilled
  const barStart = labelEnd + 2;
  const barEnd = rightStart - 2;
  const barWidth = barEnd - barStart;

  if (barWidth > 4) {
    const total = snap.featureProgress.total;
    const pct = total > 0
      ? (snap.featureProgress.done + snap.featureProgress.active) / total
      : 0;
    const filled = Math.round(Math.min(1, pct) * barWidth);

    // Filled portion: colored background with spaces (no gaps)
    for (let i = 0; i < filled; i++) {
      buf.set(y, barStart + i, " ", { bg: statusColor });
    }
    // Unfilled portion: dark background
    for (let i = filled; i < barWidth; i++) {
      buf.set(y, barStart + i, " ", { bg: 238 });
    }
  }

  buf.writeText(y, rightStart, rightText, { fg: PALETTE.brightWhite, bold: true });
}
