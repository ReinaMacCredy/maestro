/**
 * Progress log panel -- "Progress Log" header, reverse-chronological events
 * with relative age timestamps like "<1m ago", "5m ago".
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlEvent } from "../types.js";
import { PALETTE } from "../theme.js";
import { formatAge, truncate } from "../format.js";

export function renderProgressLog(
  buf: Buffer,
  rect: Rect,
  events: readonly MissionControlEvent[],
): void {
  const w = rect.width - 2;
  let row = rect.y;
  const maxRow = rect.y + rect.height;

  // Section header
  buf.writeText(row, rect.x + 1, "Progress Log", { fg: PALETTE.brightWhite, bold: true });
  row += 2;

  if (events.length === 0) {
    buf.writeText(row, rect.x + 1, "No events yet", { fg: PALETTE.dimGray });
    return;
  }

  const nowMs = Date.now();

  for (let i = 0; i < events.length && row < maxRow; i++) {
    const evt = events[i]!;
    const age = formatAge(new Date(evt.timestamp).getTime(), nowMs);
    const ageCol = rect.x + 1;
    const titleCol = rect.x + 10;

    // Age label (right-aligned in 8-char column)
    buf.writeText(row, ageCol + (8 - age.length), age, { fg: PALETTE.dimGray });

    // Event title with colored feature/checkpoint IDs
    const titleWidth = w - 10;
    buf.writeText(row, titleCol, truncate(evt.title, titleWidth), {
      fg: evt.kind === "feature" || evt.kind === "worker" ? PALETTE.green : PALETTE.gray,
    });
    row++;
  }
}
