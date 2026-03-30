/**
 * Header panel -- .:. Mission Control ~ with TIME and token counters.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { PALETTE } from "../theme.js";
import { formatElapsed, formatTokens } from "../format.js";

export function renderHeader(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const w = rect.width;
  const y = rect.y;
  buf.fillRect(rect, " ", { bg: PALETTE.headerBg });

  // Animated dots + title
  buf.writeText(y, rect.x + 1, ".:.", { fg: PALETTE.orange, bg: PALETTE.headerBg });
  buf.writeText(y, rect.x + 5, "Mission Control", { fg: PALETTE.orange, bg: PALETTE.headerBg, bold: true });
  buf.writeText(y, rect.x + 21, "~", { fg: PALETTE.dimGray, bg: PALETTE.headerBg });

  // Right side: TIME + token counters
  const parts: string[] = [];
  parts.push(`TIME ${formatElapsed(snap.elapsedMs)}`);

  if (snap.tokenCounters) {
    parts.push(`Input ${formatTokens(snap.tokenCounters.input)}`);
    parts.push(`Cached ${formatTokens(snap.tokenCounters.cached)}`);
    parts.push(`Output ${formatTokens(snap.tokenCounters.output)}`);
  }

  const rightText = parts.join("  \u00b7  "); // · separator
  const rx = rect.x + w - rightText.length - 1;
  if (rx > 24) {
    buf.writeText(y, rx, rightText, { fg: PALETTE.gray, bg: PALETTE.headerBg });
  }
}
