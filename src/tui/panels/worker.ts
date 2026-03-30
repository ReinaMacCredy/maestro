/**
 * Worker panel -- "Active Worker #{n} {type}" header with Duration,
 * shows worker report or placeholder.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { PALETTE } from "../theme.js";
import { formatElapsed, truncate } from "../format.js";

export function renderWorkerPanel(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const y = rect.y;
  const w = rect.width;

  // Border top
  for (let c = rect.x; c < rect.x + w; c++) {
    buf.set(y, c, "\u2500", { fg: PALETTE.dimGray });
  }

  if (!snap.activeWorker) {
    buf.writeText(y + 1, rect.x + 1, "No active workers", { fg: PALETTE.dimGray });
    return;
  }

  const aw = snap.activeWorker;

  // Header: "Active Worker  #1  {workerType}"
  let col = rect.x + 1;
  col += buf.writeText(y + 1, col, "Active Worker", { fg: PALETTE.brightWhite, bold: true });
  col += buf.writeText(y + 1, col + 1, " #1", { fg: PALETTE.dimGray });
  col += buf.writeText(y + 1, col + 2, ` ${aw.workerType}`, { fg: PALETTE.gray });

  // Duration on right
  const duration = `Duration ${formatElapsed(aw.elapsedMs)}`;
  buf.writeText(y + 1, rect.x + w - duration.length - 1, duration, { fg: PALETTE.gray });

  // Worker output area (rows y+2 to y+height-1)
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
      buf.writeText(outputStart, rect.x + 1, "Worker active...", { fg: PALETTE.yellow });
    }
  }
}
