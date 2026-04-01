/**
 * Header panel -- animated 3-dot mark, title, and runtime counters.
 */
import cliSpinners from "cli-spinners";
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../state/types.js";
import { PALETTE } from "../theme.js";
import { formatElapsed, formatTokens } from "../format.js";

const HEADER_DOT_FRAMES = ["●••", "•●•", "••●", "•●•"] as const;

export const HEADER_DOT_INTERVAL_MS = cliSpinners.orangePulse.interval;

export function isHeaderAnimationActive(snap: MissionControlSnapshot): boolean {
  return snap.mode === "mission"
    && (snap.effectiveStatus === "executing" || snap.effectiveStatus === "validating");
}

export function getHeaderDotsFrame(
  snap: MissionControlSnapshot,
  frameIndex: number,
): string {
  if (!isHeaderAnimationActive(snap)) {
    return HEADER_DOT_FRAMES[0];
  }

  return HEADER_DOT_FRAMES[Math.abs(frameIndex) % HEADER_DOT_FRAMES.length] ?? HEADER_DOT_FRAMES[0];
}

export function renderHeader(
  buf: Buffer,
  rect: Rect,
  snap: MissionControlSnapshot,
  animationFrame = 0,
): void {
  const w = rect.width;
  const y = rect.y;
  buf.fillRect(rect, " ", { bg: PALETTE.headerBg });

  // Animated dots + title
  const dots = getHeaderDotsFrame(snap, animationFrame);
  for (let i = 0; i < dots.length; i++) {
    const char = dots[i] ?? "•";
    const isActive = char === "●";
    buf.set(y, rect.x + 1 + i, char, {
      fg: isActive ? PALETTE.orange : PALETTE.dimGray,
      bg: PALETTE.headerBg,
      bold: isActive,
      dim: !isActive,
    });
  }
  buf.writeText(y, rect.x + 5, "Mission Control", { fg: PALETTE.orange, bg: PALETTE.headerBg, bold: true });

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
