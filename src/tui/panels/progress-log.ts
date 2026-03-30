/**
 * Progress log panel -- reverse-chronological event list with relative timestamps.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlEvent } from "../types.js";
import { PALETTE } from "../theme.js";
import { formatRelativeTime, truncate } from "../format.js";

export function renderProgressLog(
  buf: Buffer,
  rect: Rect,
  events: readonly MissionControlEvent[],
): void {
  const w = rect.width - 2;
  let row = rect.y;
  const maxRow = rect.y + rect.height;

  if (events.length === 0) {
    buf.writeText(row, rect.x + 1, "No events yet", { fg: PALETTE.dimGray });
    return;
  }

  const baseMs = new Date(events[events.length - 1]?.timestamp ?? "").getTime();

  for (let i = 0; i < events.length && row < maxRow; i++) {
    const evt = events[i]!;
    const time = formatRelativeTime(new Date(evt.timestamp).getTime(), baseMs);
    const line = `${time}  ${truncate(evt.title, w - 8)}`;
    buf.writeText(row, rect.x + 1, line, { fg: PALETTE.dimGray });
    row++;
  }
}
