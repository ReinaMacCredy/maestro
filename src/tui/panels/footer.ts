/**
 * Footer panel -- key binding hints.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { PALETTE } from "../theme.js";
import { truncate } from "../format.js";

export function renderFooter(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const w = rect.width;
  const y = rect.y;
  buf.fillRect(rect, " ", { fg: PALETTE.brightWhite, bg: 236 });

  const parts: string[] = ["q Quit", "Up/Dn Navigate", "Enter Action"];
  if (snap.canPause) parts.push("P Pause");
  if (snap.canResume) parts.push("P Resume");
  parts.push("D Dir");

  const footer = " " + parts.join("  ");
  buf.writeText(y, rect.x, truncate(footer, w), { fg: PALETTE.brightWhite, bg: 236 });
}
