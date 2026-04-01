/**
 * Status bar panel -- ● RUNNING [green][dark] N/M [+K]
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type {
  MissionControlSnapshot,
  MissionControlStatusProgress,
  MissionControlMilestoneRow,
} from "../state/types.js";
import {
  MILESTONE_KIND_COLOR,
  MILESTONE_KIND_INDICATOR,
  MILESTONE_PROFILE_COLOR,
  MILESTONE_PROFILE_LABEL,
  MISSION_STATUS_COLOR,
  MISSION_STATUS_LABEL,
  PALETTE,
  DOT_FILLED,
} from "../theme.js";

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
        buf.writeText(y, summaryX, summary, { fg: PALETTE.overlayHint });
      }
    return;
  }

  const statusColor = MISSION_STATUS_COLOR[snap.effectiveStatus] ?? PALETTE.gray;
  const label = MISSION_STATUS_LABEL[snap.effectiveStatus] ?? snap.effectiveStatus.toUpperCase();
  const summary = buildMissionSummary(snap.statusProgress, Math.max(0, w - 24));

  // ● STATUS
  buf.writeText(y, rect.x + 1, DOT_FILLED, { fg: statusColor, bold: true });
  buf.writeText(y, rect.x + 3, label, { fg: statusColor, bold: true });

  // Counts on right
  const rightText = summary;
  const rightStart = rect.x + w - rightText.length - 1;
  let labelEnd = rect.x + 3 + label.length;

  const activeMilestone = getActiveMilestone(snap);
  if (activeMilestone) {
    labelEnd = renderActiveMilestone(buf, y, labelEnd + 2, rightStart - 2, snap, activeMilestone);
  }

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

  if (rect.height > 1) {
    renderMilestoneMetaRow(buf, { x: rect.x, y: y + 1, width: w, height: rect.height - 1 }, snap, activeMilestone);
  }
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

function getActiveMilestone(snap: MissionControlSnapshot): MissionControlMilestoneRow | undefined {
  return snap.milestones.find((milestone) =>
    milestone.status === "executing" || milestone.status === "validating");
}

function renderActiveMilestone(
  buf: Buffer,
  y: number,
  startX: number,
  maxX: number,
  snap: MissionControlSnapshot,
  milestone: MissionControlMilestoneRow,
): number {
  if (startX > maxX) {
    return startX - 2;
  }

  let cursor = startX;
  const kind = milestone.kind ?? "work";
  const profile = milestone.profile ?? "custom";
  const indicator = MILESTONE_KIND_INDICATOR[kind];
  const profileLabel = MILESTONE_PROFILE_LABEL[profile];
  const detail = snap.gateBlocked
    ? `${snap.gateLabel ?? milestone.title} BLOCKED`
    : milestone.title;

  cursor = writeSegment(buf, y, cursor, maxX, `${indicator} `, {
    fg: MILESTONE_KIND_COLOR[kind],
    bold: true,
  });
  cursor = writeSegment(buf, y, cursor, maxX, profileLabel, {
    fg: MILESTONE_PROFILE_COLOR[profile],
    bold: true,
  });
    cursor = writeSegment(buf, y, cursor, maxX, ` ${detail}`, {
      fg: snap.gateBlocked ? PALETTE.red : PALETTE.overlayHint,
      bold: snap.gateBlocked,
    });

  return cursor - 1;
}

function writeSegment(
  buf: Buffer,
  y: number,
  startX: number,
  maxX: number,
  text: string,
  style: { fg: number; bold?: boolean },
): number {
  if (startX > maxX) {
    return startX;
  }

  const width = maxX - startX + 1;
  const clipped = text.length > width ? text.slice(0, width) : text;
  buf.writeText(y, startX, clipped, style);
  return startX + clipped.length;
}

function renderMilestoneMetaRow(
  buf: Buffer,
  rect: Rect,
  snap: MissionControlSnapshot,
  activeMilestone?: MissionControlMilestoneRow,
): void {
  const milestoneText = `Milestone: ${activeMilestone?.title ?? "--"}`;
  const gateValue = snap.gateBlocked
    ? snap.gateLabel ?? activeMilestone?.title ?? "blocked"
    : "clear";
  const gateText = `Gate: ${gateValue}`;

    buf.writeText(rect.y, rect.x + 1, milestoneText, { fg: PALETTE.overlayHint });
  const gateX = rect.x + rect.width - gateText.length - 1;
  if (gateX > rect.x + milestoneText.length + 4) {
      buf.writeText(rect.y, gateX, gateText, {
        fg: snap.gateBlocked ? PALETTE.red : PALETTE.overlayHint,
        bold: snap.gateBlocked,
      });
  }
}
