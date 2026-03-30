/**
 * Feature detail panel (left pane) -- metadata, preconditions, verification steps.
 */
import type { Buffer, Cell } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { FEATURE_STATUS_COLOR, PALETTE } from "../theme.js";
import { truncate } from "../format.js";

export function renderFeatureDetail(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  if (!snap.activeFeature) {
    buf.writeText(rect.y + 1, rect.x + 1, "No active feature", { fg: PALETTE.dimGray });
    return;
  }

  const f = snap.activeFeature;
  const w = rect.width - 2;
  let row = rect.y;
  const maxRow = rect.y + rect.height;

  const writeLine = (text: string, style?: Partial<Cell>): void => {
    if (row >= maxRow) return;
    buf.writeText(row, rect.x + 1, truncate(text, w), style);
    row++;
  };

  writeLine(`Skill: ${f.workerType}`, { fg: PALETTE.cyan });
  writeLine(`Milestone: ${f.milestoneId} - ${f.milestoneTitle}`, { fg: PALETTE.gray });
  writeLine(`Status: ${f.status}`, { fg: FEATURE_STATUS_COLOR[f.status] });
  row++;

  if (f.description && row < maxRow - 4) {
    writeLine("Description", { fg: PALETTE.brightWhite, bold: true });
    const descLines = f.description.split("\n").slice(0, 3);
    for (const line of descLines) {
      writeLine(`  ${line}`, { fg: PALETTE.gray });
    }
    row++;
  }

  if (f.preconditions && row < maxRow - 3) {
    writeLine("Preconditions", { fg: PALETTE.brightWhite, bold: true });
    writeLine(`  ${truncate(f.preconditions, w - 2)}`, { fg: PALETTE.gray });
    row++;
  }

  if (f.expectedBehavior && row < maxRow - 3) {
    writeLine("Expected Behavior", { fg: PALETTE.brightWhite, bold: true });
    writeLine(`  ${truncate(f.expectedBehavior, w - 2)}`, { fg: PALETTE.gray });
    row++;
  }

  if (f.verificationSteps.length > 0 && row < maxRow - 2) {
    writeLine("Verification Steps", { fg: PALETTE.brightWhite, bold: true });
    for (let i = 0; i < Math.min(f.verificationSteps.length, maxRow - row - 1); i++) {
      writeLine(`  ${i + 1}. ${truncate(f.verificationSteps[i]!, w - 6)}`, { fg: PALETTE.gray });
    }
  }
}
