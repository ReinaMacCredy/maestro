/**
 * Modal overlay widget -- centered command-palette surface with shared layout
 * for selectable menus and informational detail cards.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import { PALETTE } from "../theme.js";
import { truncate } from "../format.js";

type ModalTone = "default" | "muted" | "accent";
type ModalStyle = "plain" | "block";

export interface ModalRow {
  label?: string;
  text?: string;
  detail?: string;
  hint?: string;
  section?: string;
  tone?: ModalTone;
  style?: ModalStyle;
}

export interface ModalInfoItem extends ModalRow {
  text: string;
}

export interface MenuModalOptions {
  mode: "menu";
  title: string;
  eyebrow?: string;
  items: readonly (string | ModalRow)[];
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

export interface PaletteModalOptions {
  mode: "palette";
  title: string;
  query: string;
  items: readonly ModalRow[];
  selectedIndex: number;
  footer?: string;
  emptyLabel?: string;
}

export type ModalOptions = MenuModalOptions | InfoModalOptions | PaletteModalOptions;

export interface ModalLayout extends Rect {
  readonly contentRect: Rect;
  readonly footerRect: Rect | undefined;
  readonly itemRects: readonly Rect[];
}

interface NormalizedModalRow {
  label: string;
  detail?: string;
  hint?: string;
  section?: string;
  tone: ModalTone;
  style: ModalStyle;
}

export function pointInRect(rect: Rect, x: number, y: number): boolean {
  return x >= rect.x
    && x < rect.x + rect.width
    && y >= rect.y
    && y < rect.y + rect.height;
}

export function applyModalBackdrop(buf: Buffer, rect: Rect = {
  x: 0,
  y: 0,
  width: buf.width,
  height: buf.height,
}): void {
  for (let row = rect.y; row < rect.y + rect.height; row++) {
    for (let col = rect.x; col < rect.x + rect.width; col++) {
        const cell = buf.getCell(row, col);
        if (!cell) continue;

        buf.set(row, col, cell.char, {
          fg: dimColor(cell.fg),
          bg: PALETTE.overlayBackdropBg,
          bold: false,
          dim: true,
        });
      }
  }
}

export function layoutModal(parent: Rect, opts: ModalOptions): ModalLayout {
  const rows = normalizeRows(opts);
  const headerHeight = getHeaderHeight(opts);
  const footerHeight = opts.footer ? 2 : 1;
  const compactRows = shouldUseCompactRows(parent.height, headerHeight, footerHeight, rows);
  const emptyContentHeight = opts.mode === "palette" && rows.length === 0 ? 1 : 0;
  const contentHeight = Math.max(
    rows.reduce((height, row, index) => {
      const sectionHeight = !compactRows && row.section && row.section !== rows[index - 1]?.section ? 1 : 0;
      return height + sectionHeight + getRowHeight(row, compactRows);
    }, 0),
    emptyContentHeight,
  );

  const maxLineLength = Math.max(
    opts.title.length + 6,
    "esc".length + 6,
    opts.mode === "palette"
      ? Math.max(18, opts.query.length + 4)
      : (opts.eyebrow?.length ?? 0),
    opts.footer?.length ?? 0,
    ...rows.flatMap((row) => [
      (row.section?.length ?? 0) + 2,
      row.label.length + (row.hint?.length ?? 0) + 4,
      row.detail?.length ?? 0,
    ]),
  );

  const preferredWidth = opts.mode === "palette" ? 64 : 50;
  const maxWidth = Math.max(28, parent.width - 4);
  const modalWidth = Math.min(Math.max(maxLineLength + 6, preferredWidth), maxWidth);
  const minContentHeight = rows.length > 0 || opts.mode === "palette" ? 1 : 0;
  const minModalHeight = headerHeight + footerHeight + minContentHeight;
  const modalHeight = Math.min(
    Math.max(headerHeight + contentHeight + footerHeight, minModalHeight),
    Math.max(minModalHeight, parent.height),
  );

  const x = parent.x + Math.floor((parent.width - modalWidth) / 2);
  const y = parent.y + Math.floor((parent.height - modalHeight) / 2);
  const contentStartY = y + headerHeight;
  const footerRect = opts.footer
    ? { x: x + 2, y: y + modalHeight - 2, width: modalWidth - 4, height: 1 }
    : undefined;
  const contentRect: Rect = {
    x: x + 2,
    y: contentStartY,
    width: modalWidth - 4,
    height: Math.max(0, (footerRect?.y ?? (y + modalHeight - 1)) - contentStartY),
  };

  const itemRects: Rect[] = [];
  if (opts.mode !== "info") {
    const selectableRows = rows;
    let currentY = contentRect.y;
    for (let index = 0; index < selectableRows.length; index++) {
      const row = selectableRows[index]!;
      const previous = selectableRows[index - 1];
      if (!compactRows && row.section && row.section !== previous?.section) {
        currentY += 1;
      }
      const remainingHeight = contentRect.y + contentRect.height - currentY;
      if (remainingHeight <= 0) break;
      const height = Math.min(getRowHeight(row, compactRows), remainingHeight);
      itemRects.push({ x: x + 1, y: currentY, width: modalWidth - 2, height });
      currentY += height;
    }
  }

  return {
    x,
    y,
    width: modalWidth,
    height: modalHeight,
    contentRect,
    footerRect,
    itemRects,
  };
}

/**
 * Render a centered overlay within the parent rect and return its layout.
 */
export function renderModal(buf: Buffer, parent: Rect, opts: ModalOptions): ModalLayout {
  const layout = layoutModal(parent, opts);
  const rows = normalizeRows(opts);
  const compactRows = shouldUseCompactRows(parent.height, getHeaderHeight(opts), opts.footer ? 2 : 1, rows);
  const surfaceBg = PALETTE.overlaySurfaceBg;
  const contentWidth = layout.contentRect.width;

  buf.fillRect(layout, " ", { bg: surfaceBg });

  const titleY = layout.y + 1;
  buf.writeText(titleY, layout.x + 2, truncate(opts.title, Math.max(0, layout.width - 10)), {
    fg: PALETTE.brightWhite,
    bg: surfaceBg,
    bold: true,
  });

  const escapeText = "esc";
  const escapeX = layout.x + layout.width - escapeText.length - 2;
  if (escapeX > layout.x + 2) {
    buf.writeText(titleY, escapeX, escapeText, {
      fg: PALETTE.overlayHint,
      bg: surfaceBg,
    });
  }

  if (opts.mode === "palette") {
    renderQueryRow(buf, layout, opts.query, rows.length);
    } else if (opts.eyebrow) {
      buf.writeText(layout.y + 2, layout.x + 2, truncate(opts.eyebrow, contentWidth), {
        fg: PALETTE.overlayHint,
        bg: surfaceBg,
      });
    }

  let rowY = layout.contentRect.y;
  if (opts.mode === "palette" && rows.length === 0) {
      const emptyLabel = truncate(opts.emptyLabel ?? "No commands match", contentWidth);
      buf.writeText(rowY, layout.x + 3, emptyLabel, {
        fg: PALETTE.overlayHint,
        bg: surfaceBg,
      });
  } else {
    for (let index = 0; index < rows.length; index++) {
      const row = rows[index]!;
      const previous = rows[index - 1];
      if (!compactRows && row.section && row.section !== previous?.section) {
        if (rowY >= layout.contentRect.y + layout.contentRect.height) break;
        buf.writeText(rowY, layout.x + 2, truncate(row.section, contentWidth), {
          fg: PALETTE.overlaySection,
          bg: surfaceBg,
          bold: true,
        });
        rowY += 1;
      }

      const rowRect = opts.mode === "info"
        ? { x: layout.x + 1, y: rowY, width: layout.width - 2, height: getRowHeight(row, compactRows) }
        : layout.itemRects[index] ?? {
          x: layout.x + 1,
          y: rowY,
          width: layout.width - 2,
          height: getRowHeight(row, compactRows),
        };
      if (rowRect.y + rowRect.height > layout.contentRect.y + layout.contentRect.height) break;

      const isSelected = opts.mode !== "info" && index === opts.selectedIndex;
      renderRow(buf, rowRect, row, isSelected, layout.width - 4);
      rowY = rowRect.y + rowRect.height;
    }
  }

  if (opts.footer && layout.footerRect) {
    buf.writeText(layout.footerRect.y, layout.footerRect.x, truncate(opts.footer, layout.footerRect.width), {
      fg: PALETTE.overlayHint,
      bg: surfaceBg,
    });
  }

  return layout;
}

function renderQueryRow(
  buf: Buffer,
  layout: ModalLayout,
  query: string,
  resultCount: number,
): void {
  const queryRect = {
    x: layout.x + 2,
    y: layout.y + 2,
    width: layout.width - 4,
    height: 1,
  };
  buf.fillRect(queryRect, " ", { bg: PALETTE.overlayQueryBg });

    const prompt = "> ";
    const queryText = query.length > 0 ? query : "Type a command";
    const queryColor = query.length > 0 ? PALETTE.brightWhite : PALETTE.overlayHint;
  buf.writeText(queryRect.y, queryRect.x + 1, prompt, {
    fg: PALETTE.overlayHint,
    bg: PALETTE.overlayQueryBg,
  });
  buf.writeText(queryRect.y, queryRect.x + 1 + prompt.length, truncate(queryText, queryRect.width - 6), {
    fg: queryColor,
    bg: PALETTE.overlayQueryBg,
  });

  const resultsLabel = `${resultCount} result${resultCount === 1 ? "" : "s"}`;
  const resultsX = queryRect.x + queryRect.width - resultsLabel.length - 1;
  if (resultsX > queryRect.x + 8) {
    buf.writeText(queryRect.y, resultsX, resultsLabel, {
      fg: PALETTE.overlayHint,
      bg: PALETTE.overlayQueryBg,
    });
  }
}

function renderRow(
  buf: Buffer,
  rect: Rect,
  row: NormalizedModalRow,
  isSelected: boolean,
  width: number,
): void {
  const baseBg = row.style === "block" && !isSelected ? PALETTE.overlayQueryBg : PALETTE.overlaySurfaceBg;
  const bg = isSelected ? PALETTE.overlaySelectedBg : baseBg;
  const labelFg = isSelected ? PALETTE.overlaySelectedFg : getToneColor(row.tone);
    const detailFg = isSelected ? PALETTE.overlaySelectedFg : PALETTE.overlayHint;
  const hintFg = isSelected ? PALETTE.overlaySelectedFg : PALETTE.overlayHint;

  buf.fillRect(rect, " ", { bg });

  const labelX = rect.x + 2;
  const lineWidth = Math.max(0, width - 2);
  const hintWidth = row.hint?.length ?? 0;
  const hintX = hintWidth > 0 ? rect.x + rect.width - hintWidth - 2 : undefined;
  const labelMax = hintX ? Math.max(0, hintX - labelX - 1) : lineWidth;

  const labelText = row.style === "block"
    ? truncatePathTail(row.label, labelMax)
    : truncate(row.label, labelMax);

  buf.writeText(rect.y, labelX, labelText, {
    fg: labelFg,
    bg,
    bold: isSelected || row.tone === "accent" || row.style === "block",
  });

  if (row.hint && hintX && hintX > labelX + 4) {
    buf.writeText(rect.y, hintX, row.hint, {
      fg: hintFg,
      bg,
      dim: !isSelected,
    });
  }

  if (row.detail && rect.height > 1) {
    buf.writeText(rect.y + 1, labelX, truncate(row.detail, lineWidth), {
      fg: detailFg,
      bg,
    });
  }
}

function normalizeRows(opts: ModalOptions): NormalizedModalRow[] {
  if (opts.mode === "info") {
    return opts.items.map((item) => ({
      label: item.text,
      detail: item.detail,
      hint: item.hint,
      section: item.section,
      tone: item.tone ?? "default",
      style: item.style ?? "plain",
    }));
  }

  return opts.items.map((item) => {
    if (typeof item === "string") {
      return {
        label: item,
        tone: "default" as const,
        style: "plain" as const,
      };
    }
    return {
      label: item.label ?? item.text ?? "",
      detail: item.detail,
      hint: item.hint,
      section: item.section,
      tone: item.tone ?? "default",
      style: item.style ?? "plain",
    };
  });
}

function getHeaderHeight(opts: ModalOptions): number {
  if (opts.mode === "palette") return 4;
  return opts.eyebrow ? 4 : 3;
}

function getRowHeight(row: NormalizedModalRow, compact = false): number {
  if (compact) return 1;
  if (row.detail) return 2;
  if (row.style === "block") return 2;
  return 1;
}

function getToneColor(tone: ModalTone): number {
  if (tone === "accent") return PALETTE.brightWhite;
  if (tone === "muted") return PALETTE.overlayHint;
  return PALETTE.brightWhite;
}

function dimColor(color: number): number {
  switch (color) {
    case PALETTE.default:
      return PALETTE.gray;
    case PALETTE.brightWhite:
      return PALETTE.gray;
    case PALETTE.gray:
      return PALETTE.dimGray;
    case PALETTE.cyan:
    case PALETTE.orange:
    case PALETTE.yellow:
    case PALETTE.amber:
    case PALETTE.green:
    case PALETTE.brightGreen:
    case PALETTE.red:
    case PALETTE.blue:
    case PALETTE.magenta:
      return PALETTE.gray;
    default:
      return PALETTE.dimGray;
  }
}

function truncatePathTail(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text;
  if (maxLen <= 6) return truncate(text, maxLen);

  const tail = text.slice(-(maxLen - 3));
  return `...${tail}`;
}

function shouldUseCompactRows(
  parentHeight: number,
  headerHeight: number,
  footerHeight: number,
  rows: readonly NormalizedModalRow[],
): boolean {
  return headerHeight + footerHeight + rows.length > parentHeight;
}
