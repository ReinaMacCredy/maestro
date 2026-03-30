/**
 * Header panel -- mission title + token counters.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { PALETTE } from "../theme.js";
import { formatTokens, truncate } from "../format.js";

export function renderHeader(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const w = rect.width;
  buf.fillRect(rect, " ", { fg: PALETTE.brightWhite, bg: 236 });

  const title = `:.: Mission Control ~ ${snap.missionTitle}`;
  buf.writeText(rect.y, rect.x + 1, truncate(title, w - 2), {
    fg: PALETTE.brightWhite,
    bg: 236,
    bold: true,
  });

  // Token counters on right
  if (snap.tokenCounters) {
    const tokens = `In: ${formatTokens(snap.tokenCounters.input)}  Cache: ${formatTokens(snap.tokenCounters.cached)}  Out: ${formatTokens(snap.tokenCounters.output)}`;
    const tx = rect.x + w - tokens.length - 2;
    if (tx > rect.x + title.length + 4) {
      buf.writeText(rect.y, tx, tokens, { fg: PALETTE.gray, bg: 236 });
    }
  }
}
