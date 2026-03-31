/**
 * Status bar panel -- ● RUNNING [green][dark] N/M [+K]
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot, MissionControlStatusProgress } from "../types.js";
import { MISSION_STATUS_COLOR, MISSION_STATUS_LABEL, PALETTE, DOT_FILLED } from "../theme.js";

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
  const summary = buildMissionSummary(snap.statusProgress, Math.max(0, w - 24));

  // ● STATUS
  buf.writeText(y, rect.x + 1, DOT_FILLED, { fg: statusColor, bold: true });
  const labelEnd = rect.x + 3 + label.length;
  buf.writeText(y, rect.x + 3, label, { fg: statusColor, bold: true });

  // Counts on right
  const rightText = summary;
  const rightStart = rect.x + w - rightText.length - 1;

  // Progress bar: completion only, active work stays in the text summary.
  const barStart = labelEnd + 2;
  const barEnd = rightStart - 2;
  const barWidth = barEnd - barStart;

  if (barWidth > 4) {
    const filled = Math.round(Math.min(100, Math.max(0, snap.statusProgress.completionPct)) / 100 * barWidth);

    // Filled portion: colored background with spaces (no gaps)
    for (let i = 0; i < filled; i++) {
      buf.set(y, barStart + i, " ", { bg: statusColor });
    }
    // Unfilled portion: dark background
    for (let i = filled; i < barWidth; i++) {
      buf.set(y, barStart + i, " ", { bg: PALETTE.progressUnfilledBg });
    }
  }

  buf.writeText(y, rightStart, rightText, { fg: PALETTE.brightWhite, bold: true });
}

function buildMissionSummary(progress: MissionControlStatusProgress, maxLen: number): string {
  const segments = [`${progress.completed}/${progress.total} done`];
  if (progress.inFlight > 0) segments.push(`${progress.inFlight} active`);
  if (progress.blocked > 0) segments.push(`${progress.blocked} blocked`);
  if (progress.queued > 0) segments.push(`${progress.queued} queued`);

  let summary = segments[0] ?? "";
  for (let i = 1; i < segments.length; i++) {
    const next = `${summary}  ${segments[i]}`;
    if (maxLen > 0 && next.length > maxLen) break;
    summary = next;
  }
  return summary;
}
