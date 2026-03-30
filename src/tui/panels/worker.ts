/**
 * Worker panel -- active worker card with status and report excerpt.
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
  buf.writeText(y + 1, rect.x + 1, `Worker: ${aw.workerType}`, { fg: PALETTE.cyan });
  const duration = `Duration: ${formatElapsed(aw.elapsedMs)}`;
  buf.writeText(y + 1, rect.x + w - duration.length - 1, duration, { fg: PALETTE.gray });

  const featureLine = `Feature: ${aw.featureId} - ${aw.featureTitle}`;
  buf.writeText(y + 2, rect.x + 1, truncate(featureLine, w - 2), { fg: PALETTE.gray });

  if (aw.report) {
    const summary = truncate(aw.report.salientSummary ?? "Completed", w - 12);
    buf.writeText(y + 2, rect.x + w - summary.length - 1, summary, { fg: PALETTE.green });
  } else {
    buf.writeText(y + 2, rect.x + w - 18, "Worker active...", { fg: PALETTE.yellow });
  }
}
