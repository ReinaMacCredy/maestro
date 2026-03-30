/**
 * Feature list panel (right pane top).
 * "Features {done}/{total}" header, ●/○ dots, selected row highlight.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { FEATURE_STATUS_COLOR, PALETTE, featureDot } from "../theme.js";
import { truncate } from "../format.js";

export function renderFeatureList(
  buf: Buffer,
  rect: Rect,
  snap: MissionControlSnapshot,
  selectedIndex: number,
): void {
  const w = rect.width - 2;
  let row = rect.y;
  const maxRow = rect.y + rect.height;

  // Section header: "Features  {done}/{total}"
  buf.writeText(row, rect.x + 1, "Features", { fg: PALETTE.brightWhite, bold: true });
  const countStr = `${snap.featureProgress.done}/${snap.featureProgress.total}`;
  buf.writeText(row, rect.x + w - countStr.length, countStr, { fg: PALETTE.gray });
  row += 2;

  for (let i = 0; i < snap.features.length && row < maxRow; i++) {
    const f = snap.features[i]!;
    const isSelected = i === selectedIndex;
    const dot = featureDot(f.status);
    const dotColor = FEATURE_STATUS_COLOR[f.status];

    // Selected row gets highlight bg
    if (isSelected) {
      buf.fillRect({ x: rect.x, y: row, width: rect.width, height: 1 }, " ", { bg: PALETTE.selectedBg });
    }

    const rowStyle = isSelected ? { bg: PALETTE.selectedBg } : {};

    // Dot + title
    buf.writeText(row, rect.x + 2, dot, { fg: dotColor, ...rowStyle });
    buf.writeText(row, rect.x + 4, truncate(f.title, w - 4), {
      fg: isSelected ? PALETTE.brightWhite : PALETTE.gray,
      ...rowStyle,
    });
    row++;
  }
}
