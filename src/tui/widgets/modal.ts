/**
 * Modal overlay widget -- centered box with title, options, and selection.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import { PALETTE } from "../theme.js";
import { truncate } from "../format.js";

export interface ModalInfoItem {
  text: string;
  tone?: "default" | "muted" | "accent";
  style?: "plain" | "block";
}

export interface MenuModalOptions {
  mode: "menu";
  title: string;
  eyebrow?: string;
  items: readonly string[];
  selectedIndex: number;
  footer?: string;
}

export interface InfoModalOptions {
  mode: "info";
  title: string;
  eyebrow?: string;
  items: readonly ModalInfoItem[];
  footer?: string;
}

export type ModalOptions = MenuModalOptions | InfoModalOptions;

export interface ModalLayout extends Rect {
  readonly contentRect: Rect;
  readonly footerRect: Rect | undefined;
  readonly itemRects: readonly Rect[];
}

export function pointInRect(rect: Rect, x: number, y: number): boolean {
  return x >= rect.x
    && x < rect.x + rect.width
    && y >= rect.y
    && y < rect.y + rect.height;
}

export function layoutModal(parent: Rect, opts: ModalOptions): ModalLayout {
  const footer = opts.footer;
  const lines = opts.mode === "menu"
    ? opts.items.map((item) => item.length + 2)
    : opts.items.map((item) => item.text.length);
  const maxItemLen = Math.max(
    opts.title.length,
    opts.eyebrow?.length ?? 0,
    ...lines,
    footer?.length ?? 0,
  );
  const modalWidth = Math.min(Math.max(maxItemLen + 8, 32), parent.width - 4);
  const contentHeight = opts.mode === "menu"
    ? opts.items.length
    : opts.items.reduce((height, item) => height + (item.style === "block" ? 2 : 1), 0);
  const headerHeight = opts.eyebrow ? 3 : 2;
  const footerHeight = footer ? 2 : 1;
  const modalHeight = Math.min(headerHeight + contentHeight + footerHeight + 2, parent.height - 2);

  const x = parent.x + Math.floor((parent.width - modalWidth) / 2);
  const y = parent.y + Math.floor((parent.height - modalHeight) / 2);
  const layout: ModalLayout = {
    x,
    y,
    width: modalWidth,
    height: modalHeight,
    contentRect: {
      x: x + 2,
      y: y + headerHeight + 1,
      width: modalWidth - 4,
      height: Math.max(0, contentHeight),
    },
    footerRect: footer
      ? { x: x + 2, y: y + modalHeight - 2, width: modalWidth - 4, height: 1 }
      : undefined,
    itemRects: [],
  };

  if (opts.mode === "menu") {
    const itemRects: Rect[] = [];
    let row = layout.contentRect.y;
    for (let i = 0; i < opts.items.length; i++) {
      if (row >= y + modalHeight - 2) break;
      itemRects.push({ x: x + 1, y: row, width: modalWidth - 2, height: 1 });
      row++;
    }
    layout.itemRects = itemRects;
  }

  return layout;
}

/**
 * Render a centered modal overlay within the parent rect.
 * Returns the rect consumed by the modal.
 */
export function renderModal(buf: Buffer, parent: Rect, opts: ModalOptions): ModalLayout {
  const modalBg = PALETTE.panelBg;
  const layout = layoutModal(parent, opts);
  const footer = opts.footer;
  const mx = layout.x;
  const my = layout.y;
  const modalWidth = layout.width;
  const modalHeight = layout.height;
  const headerHeight = opts.eyebrow ? 3 : 2;

  // Clear area and draw border
  buf.fillRect(layout, " ", { bg: modalBg });
  buf.drawBorder(layout, { fg: PALETTE.border, bg: modalBg });

  // Title
  const titleText = truncate(opts.title, modalWidth - 4);
  const titleX = mx + 2;
  buf.writeText(my + 1, titleX, titleText, {
    fg: PALETTE.brightWhite,
    bg: modalBg,
    bold: true,
  });

  if (opts.eyebrow) {
    buf.writeText(my + 2, titleX, truncate(opts.eyebrow, modalWidth - 4), {
      fg: PALETTE.dimGray,
      bg: modalBg,
    });
  }

  const innerWidth = modalWidth - 4;
  let row = layout.contentRect.y;

  if (opts.mode === "menu") {
    for (let i = 0; i < opts.items.length; i++) {
      if (row >= my + modalHeight - 2) break;
      const isSelected = i === opts.selectedIndex;
      const text = truncate(opts.items[i]!, innerWidth - 4);
      const itemBg = isSelected ? PALETTE.selectedBg : modalBg;

      if (isSelected) {
        buf.fillRect({ x: mx + 1, y: row, width: modalWidth - 2, height: 1 }, " ", { bg: itemBg });
      }

      buf.writeText(row, mx + 2, isSelected ? "> " : "  ", {
        fg: isSelected ? PALETTE.brightWhite : PALETTE.dimGray,
        bg: itemBg,
        bold: isSelected,
      });
      buf.writeText(row, mx + 4, text, {
        fg: isSelected ? PALETTE.brightWhite : PALETTE.gray,
        bg: itemBg,
        bold: isSelected,
      });
      row++;
    }
  } else {
    for (const item of opts.items) {
      if (row >= my + modalHeight - 2) break;
      const text = item.style === "block"
        ? truncatePathTail(item.text, innerWidth - 4)
        : truncate(item.text, innerWidth);
      const fg = item.tone === "accent"
        ? PALETTE.brightWhite
        : item.tone === "muted"
          ? PALETTE.gray
          : PALETTE.brightWhite;

      if (item.style === "block") {
        buf.fillRect({ x: mx + 2, y: row, width: modalWidth - 4, height: 1 }, " ", { bg: PALETTE.infoBg });
        buf.writeText(row, mx + 3, text, { fg, bg: PALETTE.infoBg, bold: true });
        row += 2;
        continue;
      }

      buf.writeText(row, mx + 2, text, { fg, bg: modalBg });
      row++;
    }
  }

  if (footer) {
    const footerRow = layout.footerRect?.y ?? (my + modalHeight - 2);
    buf.writeText(footerRow, mx + 2, truncate(footer, innerWidth), {
      fg: PALETTE.gray,
      bg: modalBg,
    });
  }

  return layout;
}

function truncatePathTail(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text;
  if (maxLen <= 6) return truncate(text, maxLen);

  const tail = text.slice(-(maxLen - 3));
  return `...${tail}`;
}
