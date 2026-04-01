/**
 * Modal overlay widget -- centered command-palette surface with shared layout
 * for selectable menus and informational detail cards.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import { PALETTE } from "../theme.js";
import { truncate } from "../format.js";
import { BOX } from "../terminal/ansi.js";

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

export interface SplitModalRow extends ModalRow {
  selectable?: boolean;
}

export interface SplitModalOptions {
  mode: "split";
  title: string;
  eyebrow?: string;
  items: readonly SplitModalRow[];
  selectedIndex: number;
  detailItems: readonly ModalInfoItem[];
  footer?: string;
  emptyLabel?: string;
}

export type ModalOptions = MenuModalOptions | InfoModalOptions | PaletteModalOptions | SplitModalOptions;

export interface ModalLayout extends Rect {
  readonly contentRect: Rect;
  readonly footerRect: Rect | undefined;
  readonly itemRects: readonly Rect[];
  readonly itemRowIndexes: readonly number[];
  readonly detailRect?: Rect;
}

interface NormalizedModalRow {
  label: string;
  detail?: string;
  hint?: string;
  section?: string;
  tone: ModalTone;
  style: ModalStyle;
  selectable: boolean;
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
  if (opts.mode === "split") {
    return layoutSplitModal(parent, opts);
  }

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
  const itemRowIndexes: number[] = [];
  if (opts.mode !== "info") {
    let currentY = contentRect.y;
    for (let index = 0; index < rows.length; index++) {
      const row = rows[index]!;
      const previous = rows[index - 1];
      if (!compactRows && row.section && row.section !== previous?.section) {
        currentY += 1;
      }
      const remainingHeight = contentRect.y + contentRect.height - currentY;
      if (remainingHeight <= 0) break;
      const height = Math.min(getRowHeight(row, compactRows), remainingHeight);
      if (row.selectable) {
        itemRects.push({ x: x + 1, y: currentY, width: modalWidth - 2, height });
        itemRowIndexes.push(index);
      }
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
    itemRowIndexes,
  };
}

/**
 * Render a centered overlay within the parent rect and return its layout.
 */
export function renderModal(buf: Buffer, parent: Rect, opts: ModalOptions): ModalLayout {
  const layout = layoutModal(parent, opts);
  if (opts.mode === "split") {
    renderSplitModal(buf, layout, opts);
    return layout;
  }

  const rows = normalizeRows(opts);
  const compactRows = shouldUseCompactRows(parent.height, getHeaderHeight(opts), opts.footer ? 2 : 1, rows);
  const surfaceBg = PALETTE.overlaySurfaceBg;
  const contentWidth = layout.contentRect.width;

    buf.fillRect(layout, " ", { bg: surfaceBg, fg: PALETTE.default, bold: false, dim: false });
    buf.drawBorder(layout, { fg: PALETTE.brightWhite, bg: surfaceBg, bold: false, dim: false });

  const titleY = layout.y + 1;
  const titleText = truncate(opts.title, Math.max(0, layout.width - 10));
  const titleX = opts.mode === "palette"
    ? layout.x + Math.max(2, Math.floor((layout.width - titleText.length) / 2))
    : layout.x + 2;
      buf.writeText(titleY, titleX, titleText, {
        fg: opts.mode === "palette" ? PALETTE.amber : PALETTE.brightWhite,
        bg: surfaceBg,
        bold: true,
        dim: false,
      });

  const escapeText = "esc";
  const escapeX = layout.x + layout.width - escapeText.length - 2;
  if (escapeX > layout.x + 2) {
      buf.writeText(titleY, escapeX, escapeText, {
        fg: PALETTE.overlayHint,
        bg: surfaceBg,
        dim: false,
      });
  }

  if (opts.mode === "palette") {
    renderQueryRow(buf, layout, opts.query, rows.length);
  } else if (opts.eyebrow) {
      buf.writeText(layout.y + 2, layout.x + 2, truncate(opts.eyebrow, contentWidth), {
        fg: PALETTE.overlayHint,
        bg: surfaceBg,
        dim: false,
      });
  }

  let rowY = layout.contentRect.y;
  if (opts.mode === "palette" && rows.length === 0) {
      const emptyLabel = truncate(opts.emptyLabel ?? "No commands match", contentWidth);
        buf.writeText(rowY, layout.x + 3, emptyLabel, {
          fg: PALETTE.overlayHint,
          bg: surfaceBg,
          dim: false,
        });
  } else {
    for (let index = 0; index < rows.length; index++) {
      const row = rows[index]!;
      const previous = rows[index - 1];
        if (!compactRows && row.section && row.section !== previous?.section) {
          if (rowY >= layout.contentRect.y + layout.contentRect.height) break;
          buf.writeText(rowY, layout.x + 2, truncate(row.section, contentWidth), {
            fg: opts.mode === "palette" ? PALETTE.brightWhite : PALETTE.overlaySection,
            bg: surfaceBg,
            bold: true,
          });
        rowY += 1;
      }

        const rowRect = opts.mode === "info"
          ? { x: layout.x + 1, y: rowY, width: layout.width - 2, height: getRowHeight(row, compactRows) }
          : layout.itemRects[layout.itemRowIndexes.indexOf(index)] ?? {
            x: layout.x + 1,
            y: rowY,
            width: layout.width - 2,
            height: getRowHeight(row, compactRows),
        };
      if (rowRect.y + rowRect.height > layout.contentRect.y + layout.contentRect.height) break;

      const isSelected = opts.mode !== "info" && index === opts.selectedIndex;
        renderRow(buf, rowRect, row, isSelected, layout.width - 4, opts.mode);
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
  const prompt = "> ";
    const queryText = query.length > 0 ? query : "Type a command";
    const queryColor = PALETTE.brightWhite;
    buf.writeText(queryRect.y, queryRect.x + 1, prompt, {
      fg: PALETTE.brightWhite,
      bg: PALETTE.overlayQueryBg,
      bold: true,
      dim: false,
    });
    buf.writeText(queryRect.y, queryRect.x + 1 + prompt.length, truncate(queryText, queryRect.width - 6), {
      fg: queryColor,
      bg: PALETTE.overlayQueryBg,
      dim: false,
    });

  const resultsLabel = `${resultCount} result${resultCount === 1 ? "" : "s"}`;
  const resultsX = queryRect.x + queryRect.width - resultsLabel.length - 1;
    if (resultsX > queryRect.x + 8) {
      buf.writeText(queryRect.y, resultsX, resultsLabel, {
        fg: PALETTE.brightWhite,
        bg: PALETTE.overlayQueryBg,
        bold: true,
        dim: false,
      });
    }
  }

function renderRow(
  buf: Buffer,
  rect: Rect,
  row: NormalizedModalRow,
  isSelected: boolean,
  width: number,
  mode: ModalOptions["mode"],
): void {
  const baseBg = PALETTE.overlaySurfaceBg;
  const selectedBg = mode === "palette" ? PALETTE.amber : PALETTE.overlaySelectedBg;
  const selectedFg = mode === "palette" ? PALETTE.blue : PALETTE.overlaySelectedFg;
  const bg = isSelected ? selectedBg : baseBg;
  const labelFg = isSelected ? selectedFg : getToneColor(row.tone, mode);
    const detailFg = isSelected
      ? selectedFg
      : (mode === "palette" ? PALETTE.brightWhite : PALETTE.overlayHint);
    const hintFg = isSelected
      ? selectedFg
      : (mode === "palette" ? PALETTE.brightWhite : PALETTE.overlayHint);

    buf.fillRect(rect, " ", { bg, fg: PALETTE.default, bold: false, dim: false });

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
      bold: isSelected || row.tone === "accent" || row.style === "block" || mode === "palette",
      dim: false,
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
        dim: false,
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
      selectable: false,
    }));
  }

  if (opts.mode === "split") {
    return opts.items.map((item) => ({
      label: item.label ?? item.text ?? "",
      detail: item.detail,
      hint: item.hint,
      section: item.section,
      tone: item.tone ?? "default",
      style: item.style ?? "plain",
      selectable: item.selectable ?? true,
    }));
  }

  return opts.items.map((item) => {
    if (typeof item === "string") {
      return {
        label: item,
        tone: "default" as const,
        style: "plain" as const,
        selectable: true,
      };
    }
    return {
      label: item.label ?? item.text ?? "",
      detail: item.detail,
      hint: item.hint,
      section: item.section,
      tone: item.tone ?? "default",
      style: item.style ?? "plain",
      selectable: true,
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

function getToneColor(tone: ModalTone, mode: ModalOptions["mode"]): number {
  if (tone === "accent") return PALETTE.brightWhite;
  if (tone === "muted") return mode === "palette" ? PALETTE.gray : PALETTE.overlayHint;
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

function layoutSplitModal(parent: Rect, opts: SplitModalOptions): ModalLayout {
  const leftRows = normalizeRows(opts);
  const rightRows = normalizeInfoRows(opts.detailItems);
  const headerHeight = getHeaderHeight(opts);
  const footerHeight = opts.footer ? 2 : 1;
  const preferredWidth = 94;
  const maxWidth = Math.max(42, parent.width - 4);
  const modalWidth = Math.min(
    Math.max(
      preferredWidth,
      opts.title.length + 8,
      (opts.eyebrow?.length ?? 0) + 8,
      opts.footer?.length ?? 0 + 8,
      ...leftRows.flatMap((row) => [(row.section?.length ?? 0) + 4, row.label.length + 4]),
      ...rightRows.flatMap((row) => [(row.section?.length ?? 0) + 4, row.label.length + (row.detail?.length ?? 0) + 6]),
    ),
    maxWidth,
  );

  const leftPaneWidth = Math.max(20, Math.floor((modalWidth - 7) * 0.46));
  const rightPaneWidth = modalWidth - 5 - leftPaneWidth - 1;
  const leftContentHeight = Math.max(1, getPaneContentHeight(leftRows, false));
  const rightContentHeight = Math.max(1, getPaneContentHeight(rightRows, true));
  const contentHeight = Math.max(leftContentHeight, rightContentHeight);
  const minModalHeight = headerHeight + footerHeight + 6;
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
  const detailRect: Rect = {
    x: contentRect.x + leftPaneWidth + 1,
    y: contentRect.y,
    width: rightPaneWidth,
    height: contentRect.height,
  };

  const itemRects: Rect[] = [];
  const itemRowIndexes: number[] = [];
  let currentY = contentRect.y;
  for (let index = 0; index < leftRows.length; index++) {
    const row = leftRows[index]!;
    const previous = leftRows[index - 1];
    if (row.section && row.section !== previous?.section) {
      currentY += 1;
    }
    const remainingHeight = contentRect.y + contentRect.height - currentY;
    if (remainingHeight <= 0) break;
    const height = Math.min(1, remainingHeight);
    if (row.selectable) {
      itemRects.push({ x: contentRect.x, y: currentY, width: leftPaneWidth, height });
      itemRowIndexes.push(index);
    }
    currentY += height;
  }

  return {
    x,
    y,
    width: modalWidth,
    height: modalHeight,
    contentRect,
    footerRect,
    itemRects,
    itemRowIndexes,
    detailRect,
  };
}

function renderSplitModal(buf: Buffer, layout: ModalLayout, opts: SplitModalOptions): void {
  const surfaceBg = PALETTE.overlaySurfaceBg;
  const leftRows = normalizeRows(opts);
  const rightRows = normalizeInfoRows(opts.detailItems);
  const dividerX = layout.detailRect ? layout.detailRect.x - 1 : layout.x + Math.floor(layout.width / 2);

  buf.fillRect(layout, " ", { bg: surfaceBg });
  buf.drawBorder(layout, { fg: PALETTE.brightWhite, bg: surfaceBg });

  const titleY = layout.y + 1;
  const titleText = truncate(opts.title, Math.max(0, layout.width - 10));
  buf.writeText(titleY, layout.x + 2, titleText, {
    fg: PALETTE.brightWhite,
    bg: surfaceBg,
    bold: true,
  });

  const escapeText = "Esc";
  const escapeX = layout.x + layout.width - escapeText.length - 2;
  if (escapeX > layout.x + 2) {
    buf.writeText(titleY, escapeX, escapeText, {
      fg: PALETTE.overlayHint,
      bg: surfaceBg,
    });
  }

  if (opts.eyebrow) {
    buf.writeText(layout.y + 2, layout.x + 2, truncate(opts.eyebrow, layout.width - 4), {
      fg: PALETTE.overlayHint,
      bg: surfaceBg,
    });
  }

  if (layout.detailRect) {
    for (let y = layout.contentRect.y; y < layout.contentRect.y + layout.contentRect.height; y++) {
      buf.set(y, dividerX, BOX.vertical, { fg: PALETTE.border, bg: surfaceBg });
    }
  }

  renderSplitLeftRows(buf, layout, leftRows, opts.selectedIndex);
  renderSplitDetailRows(buf, layout.detailRect ?? layout.contentRect, rightRows);

  if (opts.footer && layout.footerRect) {
    buf.writeText(layout.footerRect.y, layout.footerRect.x, truncate(opts.footer, layout.footerRect.width), {
      fg: PALETTE.overlayHint,
      bg: surfaceBg,
    });
  }
}

function renderSplitLeftRows(
  buf: Buffer,
  layout: ModalLayout,
  rows: readonly NormalizedModalRow[],
  selectedIndex: number,
): void {
  const maxWidth = Math.max(0, (layout.detailRect?.x ?? (layout.x + layout.width - 2)) - layout.contentRect.x - 2);
  let rowY = layout.contentRect.y;
  let selectableIndex = 0;

  for (let index = 0; index < rows.length; index++) {
    const row = rows[index]!;
    const previous = rows[index - 1];
    if (row.section && row.section !== previous?.section) {
      if (rowY >= layout.contentRect.y + layout.contentRect.height) break;
      buf.writeText(rowY, layout.contentRect.x, truncate(row.section, maxWidth), {
        fg: PALETTE.overlaySection,
        bg: PALETTE.overlaySurfaceBg,
        bold: true,
      });
      rowY += 1;
    }

    if (rowY >= layout.contentRect.y + layout.contentRect.height) break;

    const isSelected = row.selectable && selectableIndex === selectedIndex;
    const rect = row.selectable
      ? layout.itemRects[selectableIndex] ?? { x: layout.contentRect.x, y: rowY, width: maxWidth, height: 1 }
      : { x: layout.contentRect.x, y: rowY, width: maxWidth, height: 1 };
    const label = truncate(row.label, Math.max(0, rect.width - 2));
    const bg = isSelected ? PALETTE.amber : PALETTE.overlaySurfaceBg;
    const fg = isSelected ? PALETTE.blue : getToneColor(row.tone, "menu");

    if (row.selectable) {
      buf.fillRect(rect, " ", { bg });
    }
    buf.writeText(rect.y, rect.x + (row.selectable ? 2 : 0), label, {
      fg,
      bg,
      bold: isSelected || row.tone === "accent",
    });
    if (row.selectable) {
      selectableIndex += 1;
    }
    rowY = rect.y + 1;
  }
}

function renderSplitDetailRows(buf: Buffer, rect: Rect, rows: readonly NormalizedModalRow[]): void {
  let rowY = rect.y;
  const maxWidth = rect.width;
  for (let index = 0; index < rows.length; index++) {
    const row = rows[index]!;
    const previous = rows[index - 1];
    if (row.section && row.section !== previous?.section) {
      if (rowY >= rect.y + rect.height) break;
      buf.writeText(rowY, rect.x + 2, truncate(row.section, maxWidth - 2), {
        fg: PALETTE.overlaySection,
        bg: PALETTE.overlaySurfaceBg,
        bold: true,
      });
      rowY += 1;
    }
    if (rowY >= rect.y + rect.height) break;

    const text = row.detail
      ? `${truncate(row.label, Math.max(0, Math.floor(maxWidth * 0.34) - 2))} ${truncate(row.detail, Math.max(0, maxWidth - Math.floor(maxWidth * 0.34) - 5))}`
      : truncate(row.label, maxWidth - 2);
    buf.writeText(rowY, rect.x + 2, text, {
      fg: getToneColor(row.tone, "info"),
      bg: PALETTE.overlaySurfaceBg,
      bold: row.tone === "accent" || row.style === "block",
    });
    rowY += 1;
  }
}

function normalizeInfoRows(items: readonly ModalInfoItem[]): NormalizedModalRow[] {
  return items.map((item) => ({
    label: item.text,
    detail: item.detail,
    hint: item.hint,
    section: item.section,
    tone: item.tone ?? "default",
    style: item.style ?? "plain",
    selectable: false,
  }));
}

function getPaneContentHeight(rows: readonly NormalizedModalRow[], compact: boolean): number {
  return rows.reduce((height, row, index) => {
    const sectionHeight = row.section && row.section !== rows[index - 1]?.section ? 1 : 0;
    return height + sectionHeight + getRowHeight(row, compact);
  }, 0);
}
