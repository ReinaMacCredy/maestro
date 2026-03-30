/**
 * Feature list panel (right pane) -- milestone groups, selection highlight, status dots.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { FEATURE_STATUS_COLOR, PALETTE, featureDot, FEATURE_STATUS_LABEL } from "../theme.js";
import { truncate } from "../format.js";

export function renderFeatureList(
  buf: Buffer,
  rect: Rect,
  snap: MissionControlSnapshot,
  selectedIndex: number,
): void {
  let row = rect.y;
  const w = rect.width - 2;
  const maxRow = rect.y + rect.height;
  let currentMilestone = "";

  for (let i = 0; i < snap.features.length && row < maxRow; i++) {
    const f = snap.features[i]!;

    // Milestone header
    if (f.milestoneId !== currentMilestone) {
      currentMilestone = f.milestoneId;
      const ms = snap.milestones.find((m) => m.id === f.milestoneId);
      const msTitle = ms ? `${ms.id} - ${ms.title}` : f.milestoneId;
      buf.writeText(row, rect.x + 1, truncate(msTitle, w), { fg: PALETTE.dimGray, dim: true });
      row++;
      if (row >= maxRow) break;
    }

    // Feature row
    const isSelected = i === selectedIndex;
    const dot = featureDot(f.status);
    const statusLabel = `[${FEATURE_STATUS_LABEL[f.status]}]`;
    const titleSpace = w - statusLabel.length - 8;
    const line = `${isSelected ? ">" : " "} ${dot} ${f.id.padEnd(4)} ${truncate(f.title, Math.max(titleSpace, 8))}`;

    const rowStyle = isSelected ? { bg: 237 } : {};
    buf.fillRect({ x: rect.x, y: row, width: rect.width, height: 1 }, " ", rowStyle);
    buf.writeText(row, rect.x + 1, line, { fg: FEATURE_STATUS_COLOR[f.status], ...rowStyle });
    buf.writeText(row, rect.x + rect.width - statusLabel.length - 1, statusLabel, {
      fg: FEATURE_STATUS_COLOR[f.status],
      ...rowStyle,
    });
    row++;
  }
}
