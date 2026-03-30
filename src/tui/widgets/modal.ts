/**
 * Modal overlay widget -- centered box with title, options, and selection.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import { PALETTE } from "../theme.js";
import { truncate } from "../format.js";
import { BOX } from "../terminal/ansi.js";

export interface ModalOptions {
  title: string;
  items: readonly string[];
  selectedIndex: number;
  /** Optional status line below items. */
  statusLine?: string;
}

/**
 * Render a centered modal overlay within the parent rect.
 * Returns the rect consumed by the modal.
 */
export function renderModal(buf: Buffer, parent: Rect, opts: ModalOptions): Rect {
  const { title, items, selectedIndex, statusLine } = opts;
  const modalBg = PALETTE.selectedBg;
  const selectedBg = 238;

  // Calculate modal dimensions
  const maxItemLen = Math.max(
    title.length,
    ...items.map((i) => i.length + 4), // "  > item"
    (statusLine?.length ?? 0),
  );
  const modalWidth = Math.min(Math.max(maxItemLen + 4, 20), parent.width - 4);
  const modalHeight = Math.min(items.length + 3 + (statusLine ? 1 : 0), parent.height - 2);

  // Center within parent
  const mx = parent.x + Math.floor((parent.width - modalWidth) / 2);
  const my = parent.y + Math.floor((parent.height - modalHeight) / 2);
  const modalRect: Rect = { x: mx, y: my, width: modalWidth, height: modalHeight };

  // Clear area and draw border
  buf.fillRect(modalRect, " ", { bg: modalBg });
  buf.drawBorder(modalRect, { fg: PALETTE.cyan, bg: modalBg });

  // Title
  const titleText = truncate(title, modalWidth - 4);
  const titleX = mx + Math.max(1, Math.floor((modalWidth - titleText.length) / 2));
  buf.writeText(my, titleX, titleText, {
    fg: PALETTE.brightWhite,
    bg: modalBg,
    bold: true,
  });

  // Items
  const innerWidth = modalWidth - 4;
  for (let i = 0; i < items.length; i++) {
    const row = my + 2 + i;
    if (row >= my + modalHeight - 1) break;

    const isSelected = i === selectedIndex;
    const prefix = isSelected ? "> " : "  ";
    const text = truncate(items[i]!, innerWidth - 2);
    const itemBg = isSelected ? selectedBg : modalBg;

    if (isSelected) {
      buf.fillRect({ x: mx + 1, y: row, width: modalWidth - 2, height: 1 }, " ", { bg: itemBg });
    }

    buf.writeText(row, mx + 2, prefix + text, {
      fg: isSelected ? PALETTE.brightWhite : PALETTE.gray,
      bg: itemBg,
      bold: isSelected,
    });
  }

  // Status line
  if (statusLine) {
      const statusRow = my + modalHeight - 2;
    if (statusRow > my + 1) {
      buf.writeText(statusRow, mx + 2, truncate(statusLine, innerWidth), {
        fg: PALETTE.gray,
        bg: modalBg,
      });
    }
  }

  return modalRect;
}
