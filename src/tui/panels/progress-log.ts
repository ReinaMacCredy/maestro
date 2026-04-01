/**
 * Timeline panel -- reverse-chronological events
 * with relative age timestamps like "<1m ago", "5m ago".
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlEvent, MissionControlSnapshot } from "../types.js";
import { PALETTE } from "../theme.js";
import { formatAge, truncate } from "../format.js";

export function renderProgressLog(
  buf: Buffer,
  rect: Rect,
  events: readonly MissionControlEvent[],
  snap?: MissionControlSnapshot,
  scrollOffset = 0,
): void {
  const w = rect.width - 2;
  let row = rect.y;
  const maxRow = rect.y + rect.height;

  if (snap?.mode === "home" && snap.home) {
    buf.writeText(row, rect.x + 1, "Pending Handoffs", { fg: PALETTE.brightWhite, bold: true });
    row += 2;

    if (snap.home.pendingHandoffs.length === 0) {
      buf.writeText(row, rect.x + 1, "No pending handoffs", { fg: PALETTE.gray });
      return;
    }

    for (const handoff of snap.home.pendingHandoffs) {
      if (row >= maxRow) break;
      const prefix = `${handoff.id} · ${handoff.agent}`;
      buf.writeText(row, rect.x + 1, truncate(prefix, w), { fg: PALETTE.gray });
      row++;
      if (row >= maxRow) break;
      buf.writeText(row, rect.x + 1, truncate(handoff.message, w), { fg: PALETTE.brightWhite });
      row++;
    }
    return;
  }

    // Section header
    buf.writeText(row, rect.x + 1, "Timeline", { fg: PALETTE.brightWhite, bold: true });
  row += 2;

  if (events.length === 0) {
    buf.writeText(row, rect.x + 1, "No events yet", { fg: PALETTE.gray });
    return;
  }

  const nowMs = Date.now();

  const visibleEvents = scrollOffset > 0 ? events.slice(scrollOffset) : events;

  for (let i = 0; i < visibleEvents.length && row < maxRow; i++) {
    const evt = visibleEvents[i]!;
    const age = formatAge(new Date(evt.timestamp).getTime(), nowMs);
    const ageCol = rect.x + 1;
    const titleCol = rect.x + 10;

    // Age label (right-aligned in 8-char column)
    buf.writeText(row, ageCol + (8 - age.length), age, { fg: PALETTE.gray });

      // Event title with higher-signal event coloring.
      const titleWidth = w - 10;
      buf.writeText(row, titleCol, truncate(evt.title, titleWidth), {
        fg: eventColor(evt),
      });
      row++;
    }
}

function eventColor(evt: MissionControlEvent): number {
  if (evt.kind === "worker") return PALETTE.blue;
  if (evt.kind === "checkpoint") return PALETTE.yellow;
  if (evt.kind === "feature" && evt.title.toLowerCase().includes("blocked")) return PALETTE.red;
  if (evt.kind === "feature") return PALETTE.green;
  return PALETTE.brightWhite;
}
