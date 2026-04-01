/**
 * Task list panel (right pane).
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { FEATURE_STATUS_COLOR, FEATURE_STATUS_LABEL, PALETTE } from "../theme.js";
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

  if (snap.mode === "home" && snap.home) {
    const okCount = snap.home.checks.filter((check) => check.status === "ok").length;
    buf.writeText(row, rect.x + 1, "Environment", { fg: PALETTE.brightWhite, bold: true });
    const countStr = `${okCount}/${snap.home.checks.length} ok`;
    buf.writeText(row, rect.x + w - countStr.length, countStr, { fg: PALETTE.brightWhite, bold: true });
    row += 2;

    for (const check of snap.home.checks) {
      if (row >= maxRow) break;
      const marker = check.status === "ok" ? "●" : check.status === "warn" ? "!" : "x";
      const color = check.status === "ok" ? PALETTE.green : check.status === "warn" ? PALETTE.yellow : PALETTE.red;
      buf.writeText(row, rect.x + 2, marker, { fg: color });
      buf.writeText(row, rect.x + 4, truncate(check.message, w - 4), { fg: PALETTE.brightWhite });
      row++;
    }
    return;
  }

  // Section header: "Tasks  {done}/{total}"
  buf.writeText(row, rect.x + 1, "Tasks", { fg: PALETTE.brightWhite, bold: true });
  const countStr = `${snap.featureProgress.done}/${snap.featureProgress.total}`;
  buf.writeText(row, rect.x + w - countStr.length, countStr, { fg: PALETTE.brightWhite, bold: true });
  row += 2;

  for (let i = 0; i < snap.features.length && row < maxRow; i++) {
    const f = snap.features[i]!;
    const isSelected = i === selectedIndex;
    const statusLabel = FEATURE_STATUS_LABEL[f.status];
    const statusColor = FEATURE_STATUS_COLOR[f.status];
    const blockedByText = f.blockedByLabel ? `by ${f.blockedByLabel}` : "";

    // Selected row gets highlight bg
    if (isSelected) {
      buf.fillRect({ x: rect.x, y: row, width: rect.width, height: 1 }, " ", { bg: PALETTE.selectedBg });
    }

    const rowStyle = isSelected ? { bg: PALETTE.selectedBg } : {};

    let col = rect.x + 2;
    buf.writeText(row, col, truncate(statusLabel, 10), { fg: statusColor, ...rowStyle, bold: true });
    col += 11;
    if (blockedByText) {
      buf.writeText(row, col, truncate(blockedByText, Math.max(0, w - 18)), { fg: PALETTE.red, dim: true, ...rowStyle });
      col += blockedByText.length + 1;
    }
    buf.writeText(row, col, f.id, { fg: PALETTE.brightWhite, ...rowStyle, bold: true });
    col += f.id.length + 2;
    buf.writeText(row, col, truncate(f.title, Math.max(0, rect.x + rect.width - col - 1)), {
      fg: PALETTE.brightWhite,
      ...rowStyle,
    });
    row++;
  }
}
