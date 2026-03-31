/**
 * Feature detail panel (left pane).
 * "Active Feature {name}" header, then skill/milestone, sections with bullet lists.
 */
import type { Buffer, Cell } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../types.js";
import { PALETTE } from "../theme.js";
import { truncate } from "../format.js";

const BULLET = "\u00b7"; // ·

export function renderFeatureDetail(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  if (snap.mode === "home" && snap.home) {
    renderHomeOverview(buf, rect, snap);
    return;
  }

  if (!snap.activeFeature) {
    buf.writeText(rect.y + 1, rect.x + 1, "No active feature", { fg: PALETTE.dimGray });
    return;
  }

  const f = snap.activeFeature;
  const w = rect.width - 2;
  let row = rect.y;
  const maxRow = rect.y + rect.height;
  const writers = createPanelWriters(buf, rect, w, maxRow, () => row, (nextRow) => {
    row = nextRow;
  });
  const { writeLine } = writers;
  const writeBullet = (text: string): void => {
    writers.writeBullet(text, { fg: PALETTE.gray });
  };

  const writeSection = (title: string): void => {
    if (row >= maxRow) return;
    row++; // blank line before section
    if (row >= maxRow) return;
    writeLine(title, { fg: PALETTE.brightWhite, bold: true });
  };

  // Header: "Active Feature  {name}"
  buf.writeText(row, rect.x + 1, "Active Feature", { fg: PALETTE.brightWhite, bold: true });
  buf.writeText(row, rect.x + 16, truncate(f.title, w - 16), { fg: PALETTE.gray });
  row += 2;

  // Skill + milestone
  writeLine(`skill ${f.workerType}`, { fg: PALETTE.gray });
  writeLine(`milestone ${f.milestoneId}`, { fg: PALETTE.gray });

  // Preconditions
  if (f.preconditions && row < maxRow - 4) {
    writeSection("Preconditions");
    const lines = f.preconditions.split("\n").filter((l) => l.trim());
    for (const line of lines.slice(0, 4)) {
      writeBullet(line.trim());
    }
    if (lines.length > 4) {
      writeBullet(`+${lines.length - 4} more`);
    }
  }

  // Expected Behavior
  if (f.expectedBehavior && row < maxRow - 4) {
    writeSection("Expected Behavior");
    const lines = f.expectedBehavior.split("\n").filter((l) => l.trim());
    const maxShow = Math.min(lines.length, Math.max(2, maxRow - row - 6));
    for (const line of lines.slice(0, maxShow)) {
      writeBullet(line.trim());
    }
    if (lines.length > maxShow) {
      writeBullet(`+${lines.length - maxShow} more`);
    }
  }

  // Verification Steps
  if (f.verificationSteps.length > 0 && row < maxRow - 3) {
    writeSection("Verification Steps");
    const maxShow = Math.min(f.verificationSteps.length, Math.max(2, maxRow - row - 3));
    for (let i = 0; i < maxShow; i++) {
      writeBullet(f.verificationSteps[i]!);
    }
    if (f.verificationSteps.length > maxShow) {
      writeBullet(`+${f.verificationSteps.length - maxShow} more`);
    }
  }

  // Description (at bottom, if space remains)
  if (f.description && row < maxRow - 2) {
    writeSection("Description");
    const descLines = f.description.split("\n").slice(0, maxRow - row - 1);
    for (const line of descLines) {
      writeLine(`  ${line}`, { fg: PALETTE.gray });
    }
  }
}

function renderHomeOverview(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const home = snap.home!;
  const w = rect.width - 2;
  let row = rect.y;
  const maxRow = rect.y + rect.height;
  const { writeLine, writeBullet } = createPanelWriters(buf, rect, w, maxRow, () => row, (nextRow) => {
    row = nextRow;
  });

  writeLine("Overview", { fg: PALETTE.brightWhite, bold: true });
  writeLine(home.headline, { fg: PALETTE.brightWhite });
  writeLine(home.summary, { fg: PALETTE.gray });
  row++;

  writeLine("Workspace", { fg: PALETTE.brightWhite, bold: true });
  writeBullet(home.locationLabel);
  writeBullet(`${home.pendingHandoffs.length} pending handoff${home.pendingHandoffs.length === 1 ? "" : "s"}`);
  writeBullet(`${home.actions.length} suggested next step${home.actions.length === 1 ? "" : "s"}`);
}

function createPanelWriters(
  buf: Buffer,
  rect: Rect,
  width: number,
  maxRow: number,
  getRow: () => number,
  setRow: (row: number) => void,
): {
  writeLine: (text: string, style?: Partial<Cell>) => void;
  writeBullet: (text: string, style?: Partial<Cell>) => void;
} {
  const writeLine = (text: string, style?: Partial<Cell>): void => {
    const row = getRow();
    if (row >= maxRow) return;
    buf.writeText(row, rect.x + 1, truncate(text, width), style);
    setRow(row + 1);
  };

  const writeBullet = (text: string, style?: Partial<Cell>): void => {
    const row = getRow();
    if (row >= maxRow) return;
    buf.writeText(row, rect.x + 1, `${BULLET} `, { fg: PALETTE.dimGray });
    buf.writeText(row, rect.x + 3, truncate(text, width - 2), style ?? { fg: PALETTE.gray });
    setRow(row + 1);
  };

  return { writeLine, writeBullet };
}
