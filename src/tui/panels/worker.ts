/**
 * Worker panel -- "Active Worker #{n} {type}" header with Duration,
 * shows worker report or placeholder.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { FEATURE_STATUS_COLOR, FEATURE_STATUS_LABEL, PALETTE } from "../theme.js";
import { formatElapsed, truncate } from "../format.js";

export function renderWorkerPanel(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const y = rect.y;
  const w = rect.width;
  if (rect.height <= 0 || w <= 0) return;

  buf.writeText(y, rect.x + 1, "Activity", { fg: PALETTE.brightWhite, bold: true });

  if (snap.mode === "home" && snap.home) {
    renderHomeActivity(buf, rect, snap);
    return;
  }

  if (!snap.activeWorker) {
    renderMissionIdleActivity(buf, rect, snap);
    return;
  }

  const aw = snap.activeWorker;

  // Duration on right
  const duration = `Duration ${formatElapsed(aw.elapsedMs)}`;
  buf.writeText(y, rect.x + w - duration.length - 1, duration, { fg: PALETTE.gray });
  if (rect.height > 1) {
    const meta = `${aw.featureId} · ${FEATURE_STATUS_LABEL[aw.status]} · ${aw.workerType}`;
    buf.writeText(y + 1, rect.x + 1, truncate(meta, w - 2), { fg: FEATURE_STATUS_COLOR[aw.status] });
  }
  if (rect.height > 2) {
    buf.writeText(y + 2, rect.x + 1, truncate(aw.featureTitle, w - 2), { fg: PALETTE.brightWhite });
  }

  // Worker output area (rows below the header)
  const outputStart = y + 3;
  const outputEnd = y + rect.height;

  if (aw.report) {
    // Show report summary
    if (aw.report.salientSummary && outputStart < outputEnd) {
      buf.writeText(outputStart, rect.x + 1, truncate(aw.report.salientSummary, w - 2), { fg: PALETTE.gray });
    }
    if (aw.report.whatWasImplemented && outputStart + 1 < outputEnd) {
      buf.writeText(outputStart + 1, rect.x + 1, truncate(aw.report.whatWasImplemented, w - 2), { fg: PALETTE.dimGray });
    }
  } else {
    // Placeholder when worker is active but no IPC
    if (outputStart < outputEnd) {
      buf.writeText(outputStart, rect.x + 1, `Live on ${truncate(aw.featureTitle, Math.max(0, w - 9))}`, {
        fg: PALETTE.yellow,
      });
    }
    if (outputStart + 1 < outputEnd) {
      buf.writeText(outputStart + 1, rect.x + 1, "Waiting for the first worker update...", { fg: PALETTE.gray });
    }
  }
}

function renderMissionIdleActivity(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const nextFeature = snap.activeFeature;

  if (!nextFeature) {
    if (rect.height > 1) {
      buf.writeText(rect.y + 1, rect.x + 1, "No features in this mission yet", { fg: PALETTE.gray });
    }
    if (rect.height > 2) {
      buf.writeText(rect.y + 2, rect.x + 1, "Create or import work to start live activity.", { fg: PALETTE.dimGray });
    }
    return;
  }

  const statusText = `${nextFeature.id} · ${FEATURE_STATUS_LABEL[nextFeature.status]} · ${nextFeature.workerType}`;
  if (rect.height > 1) {
    buf.writeText(rect.y + 1, rect.x + 1, "Ready for the next feature", { fg: PALETTE.gray });
  }
  if (rect.height > 2) {
    buf.writeText(rect.y + 2, rect.x + 1, truncate(statusText, rect.width - 2), {
      fg: FEATURE_STATUS_COLOR[nextFeature.status],
    });
  }
  if (rect.height > 3) {
    buf.writeText(rect.y + 3, rect.x + 1, truncate(nextFeature.title, rect.width - 2), {
      fg: PALETTE.brightWhite,
    });
  }
  if (rect.height > 4) {
    buf.writeText(rect.y + 4, rect.x + 1, "Press Enter to review transitions for this feature.", {
      fg: PALETTE.gray,
    });
  }
  if (rect.height > 5) {
    buf.writeText(rect.y + 5, rect.x + 1, "Use L for timeline updates and D for mission files.", {
      fg: PALETTE.dimGray,
    });
  }
}

function renderHomeActivity(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const home = snap.home!;
  if (rect.height > 0) {
    const statusText = "Read-only home";
    buf.writeText(rect.y, rect.x + rect.width - statusText.length - 1, statusText, { fg: PALETTE.dimGray });
  }

  let row = rect.y + 1;
  for (const action of home.actions) {
    if (row >= rect.y + rect.height) break;
    buf.writeText(row, rect.x + 1, truncate(action.command, rect.width - 2), {
      fg: PALETTE.brightWhite,
      bold: true,
    });
    row++;
    if (row >= rect.y + rect.height) break;
    buf.writeText(row, rect.x + 1, truncate(action.detail, rect.width - 2), { fg: PALETTE.gray });
    row++;
  }
}
