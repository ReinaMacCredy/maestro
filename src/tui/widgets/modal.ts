/**
 * Modal overlay widget -- centered overlay surfaces with spec-driven shell,
 * sizing, and row styling for palette, menu, info, and split-detail views.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import { truncate } from "../format.js";
import { BOX } from "../terminal/ansi.js";
import { PALETTE } from "../theme.js";

type ModalTone = "default" | "muted" | "accent";
type ModalStyle = "plain" | "block";

export type OverlaySizePreset = "standard" | "wide";
export type OverlayFamily = "palette" | "menu" | "split" | "info";
export type OverlayTextCase = "preserve" | "lower";
export type OverlayTitleAlign = "left" | "center";

export interface OverlayChromeSpec {
  titleAlign: OverlayTitleAlign;
  titleColor: number;
  escapeText: string;
}

export interface OverlaySelectionSpec {
  bg: number;
  fg: number;
  fullWidth: boolean;
}

export interface OverlayTextSpec {
  rowCase: OverlayTextCase;
  sectionCase: OverlayTextCase;
  primaryColor: number;
  detailColor: number;
  hintColor: number;
  sectionColor: number;
  mutedColor: number;
}

export interface OverlayLayoutSpec {
  preferredWidth: number;
  minWidth: number;
  preferredHeight: number;
  minHeight: number;
  splitRatio?: readonly [number, number];
}

export interface OverlayRenderSpec {
  family: OverlayFamily;
  sizePreset: OverlaySizePreset;
  chrome: OverlayChromeSpec;
  selection: OverlaySelectionSpec;
  text: OverlayTextSpec;
  layout: OverlayLayoutSpec;
}

export type OverlayModalKind =
  | "command-palette"
  | "feature-action"
  | "feature-browser"
  | "overview"
  | "dependencies"
  | "handoffs"
  | "config"
  | "processes";

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
  renderSpec: OverlayRenderSpec;
}

export interface InfoModalOptions {
  mode: "info";
  title: string;
  eyebrow?: string;
  items: readonly ModalInfoItem[];
  footer?: string;
  renderSpec: OverlayRenderSpec;
}

export interface PaletteModalOptions {
  mode: "palette";
  title: string;
  query: string;
  items: readonly ModalRow[];
  selectedIndex: number;
  footer?: string;
  emptyLabel?: string;
  renderSpec: OverlayRenderSpec;
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
  renderSpec: OverlayRenderSpec;
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

const STANDARD_LAYOUT: OverlayLayoutSpec = {
  preferredWidth: 76,
  minWidth: 64,
  preferredHeight: 20,
  minHeight: 18,
};

const WIDE_LAYOUT: OverlayLayoutSpec = {
  preferredWidth: 94,
  minWidth: 84,
  preferredHeight: 20,
  minHeight: 18,
};

const STANDARD_CHROME: OverlayChromeSpec = {
  titleAlign: "center",
  titleColor: PALETTE.brightWhite,
  escapeText: "esc",
};

const STANDARD_SELECTION: OverlaySelectionSpec = {
  bg: PALETTE.yellow,
  fg: PALETTE.headerBg,
  fullWidth: true,
};

const LEGACY_SELECTION: OverlaySelectionSpec = {
  bg: PALETTE.overlaySelectedBg,
  fg: PALETTE.overlaySelectedFg,
  fullWidth: true,
};

const STANDARD_TEXT: OverlayTextSpec = {
  rowCase: "preserve",
  sectionCase: "preserve",
  primaryColor: PALETTE.brightWhite,
  detailColor: PALETTE.overlayHint,
  hintColor: PALETTE.blue,
  sectionColor: PALETTE.overlaySection,
  mutedColor: PALETTE.gray,
};

const PALETTE_TEXT: OverlayTextSpec = {
  rowCase: "lower",
  sectionCase: "lower",
  primaryColor: PALETTE.brightWhite,
  detailColor: PALETTE.brightWhite,
  hintColor: PALETTE.blue,
  sectionColor: PALETTE.gray,
  mutedColor: PALETTE.gray,
};

export function buildOverlayRenderSpec(kind: OverlayModalKind): OverlayRenderSpec {
  switch (kind) {
    case "command-palette":
      return {
        family: "palette",
        sizePreset: "standard",
        chrome: STANDARD_CHROME,
        selection: STANDARD_SELECTION,
        text: PALETTE_TEXT,
        layout: STANDARD_LAYOUT,
      };
    case "dependencies":
      return {
        family: "split",
        sizePreset: "standard",
        chrome: STANDARD_CHROME,
        selection: STANDARD_SELECTION,
        text: STANDARD_TEXT,
        layout: { ...STANDARD_LAYOUT, splitRatio: [44, 56] },
      };
    case "processes":
      return {
        family: "split",
        sizePreset: "standard",
        chrome: STANDARD_CHROME,
        selection: STANDARD_SELECTION,
        text: STANDARD_TEXT,
        layout: { ...STANDARD_LAYOUT, splitRatio: [42, 58] },
      };
    case "handoffs":
      return {
        family: "split",
        sizePreset: "wide",
        chrome: STANDARD_CHROME,
        selection: STANDARD_SELECTION,
        text: STANDARD_TEXT,
        layout: { ...WIDE_LAYOUT, splitRatio: [36, 64] },
      };
    case "config":
      return {
        family: "info",
        sizePreset: "standard",
        chrome: STANDARD_CHROME,
        selection: STANDARD_SELECTION,
        text: STANDARD_TEXT,
        layout: STANDARD_LAYOUT,
      };
    case "overview":
      return {
        family: "info",
        sizePreset: "standard",
        chrome: STANDARD_CHROME,
        selection: STANDARD_SELECTION,
        text: STANDARD_TEXT,
        layout: STANDARD_LAYOUT,
      };
    case "feature-browser":
      return {
        family: "menu",
        sizePreset: "standard",
        chrome: STANDARD_CHROME,
        selection: STANDARD_SELECTION,
        text: STANDARD_TEXT,
        layout: STANDARD_LAYOUT,
      };
    case "feature-action":
    default:
      return {
        family: "menu",
        sizePreset: "standard",
        chrome: STANDARD_CHROME,
        selection: LEGACY_SELECTION,
        text: STANDARD_TEXT,
        layout: STANDARD_LAYOUT,
      };
  }
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
  return opts.mode === "split"
    ? layoutSplitModal(parent, opts)
    : layoutSingleModal(parent, opts);
}

export function renderModal(buf: Buffer, parent: Rect, opts: ModalOptions): ModalLayout {
  const layout = layoutModal(parent, opts);
  renderOverlayShell(buf, layout, opts);

  if (opts.mode === "split") {
    renderSplitBody(buf, layout, opts);
    return layout;
  }

  renderSingleBody(buf, layout, opts);
  return layout;
}

function layoutSingleModal(
  parent: Rect,
  opts: MenuModalOptions | InfoModalOptions | PaletteModalOptions,
): ModalLayout {
  const rows = normalizeRows(opts);
  const headerHeight = getHeaderHeight(opts);
  const footerHeight = opts.footer ? 2 : 1;
  const compactRows = shouldUseCompactRows(parent.height, headerHeight, footerHeight, rows);
  const isPalette = opts.renderSpec.family === "palette";
  const emptyContentHeight = isPalette && rows.length === 0 ? 1 : 0;
  const contentHeight = Math.max(
    rows.reduce((height, row, index) => {
      const sectionHeight = !isPalette && !compactRows && row.section && row.section !== rows[index - 1]?.section ? 1 : 0;
      return height + sectionHeight + getRowHeight(row, compactRows, opts.renderSpec.family);
    }, 0),
    emptyContentHeight,
  );

  const maxLineLength = Math.max(
    opts.title.length + 6,
    opts.renderSpec.chrome.escapeText.length + 6,
    isPalette
      ? Math.max(18, (opts.mode === "palette" ? opts.query.length : 0) + 4)
      : ("eyebrow" in opts ? (opts.eyebrow?.length ?? 0) : 0),
    opts.footer?.length ?? 0,
    ...rows.flatMap((row) => [
      isPalette ? getPaletteRowLength(row) : (row.section?.length ?? 0) + 2,
      isPalette ? 0 : row.label.length + (row.hint?.length ?? 0) + 4,
      isPalette ? 0 : (row.detail?.length ?? 0),
    ]),
  );

  const baseLayout = resolveOverlayFrame(
    parent,
    opts.renderSpec.layout,
    headerHeight,
    footerHeight,
    contentHeight,
    maxLineLength,
  );

  const itemRects: Rect[] = [];
  const itemRowIndexes: number[] = [];
  if (opts.mode !== "info") {
    let currentY = baseLayout.contentRect.y;
    for (let index = 0; index < rows.length; index++) {
      const row = rows[index]!;
      const previous = rows[index - 1];
      if (!isPalette && !compactRows && row.section && row.section !== previous?.section) {
        currentY += 1;
      }
      const remainingHeight = baseLayout.contentRect.y + baseLayout.contentRect.height - currentY;
      if (remainingHeight <= 0) break;
      const height = Math.min(getRowHeight(row, compactRows, opts.renderSpec.family), remainingHeight);
      if (row.selectable) {
        itemRects.push({ x: baseLayout.x + 1, y: currentY, width: baseLayout.width - 2, height });
        itemRowIndexes.push(index);
      }
      currentY += height;
    }
  }

  return {
    ...baseLayout,
    itemRects,
    itemRowIndexes,
  };
}

function layoutSplitModal(parent: Rect, opts: SplitModalOptions): ModalLayout {
  const leftRows = normalizeRows(opts);
  const rightRows = normalizeInfoRows(opts.detailItems);
  const headerHeight = getHeaderHeight(opts);
  const footerHeight = opts.footer ? 2 : 1;

  const maxLineLength = Math.max(
    opts.title.length + 6,
    opts.renderSpec.chrome.escapeText.length + 6,
    opts.eyebrow?.length ?? 0,
    opts.footer?.length ?? 0,
    ...leftRows.flatMap((row) => [(row.section?.length ?? 0) + 4, row.label.length + 4]),
    ...rightRows.flatMap((row) => [(row.section?.length ?? 0) + 4, row.label.length + (row.detail?.length ?? 0) + 6]),
  );

  const leftContentHeight = Math.max(1, getPaneContentHeight(leftRows));
  const rightContentHeight = Math.max(1, getPaneContentHeight(rightRows));
  const contentHeight = Math.max(leftContentHeight, rightContentHeight);

  const baseLayout = resolveOverlayFrame(
    parent,
    opts.renderSpec.layout,
    headerHeight,
    footerHeight,
    contentHeight,
    maxLineLength,
  );

  const ratio = opts.renderSpec.layout.splitRatio ?? [46, 54];
  const { leftPaneWidth, rightPaneWidth } = getSplitPaneWidths(baseLayout.contentRect.width, ratio);
  const detailRect: Rect = {
    x: baseLayout.contentRect.x + leftPaneWidth + 1,
    y: baseLayout.contentRect.y,
    width: rightPaneWidth,
    height: baseLayout.contentRect.height,
  };

  const itemRects: Rect[] = [];
  const itemRowIndexes: number[] = [];
  let currentY = baseLayout.contentRect.y;
  for (let index = 0; index < leftRows.length; index++) {
    const row = leftRows[index]!;
    const previous = leftRows[index - 1];
    if (row.section && row.section !== previous?.section) {
      currentY += 1;
    }
    const remainingHeight = baseLayout.contentRect.y + baseLayout.contentRect.height - currentY;
    if (remainingHeight <= 0) break;
    const height = Math.min(1, remainingHeight);
    if (row.selectable) {
      itemRects.push({ x: baseLayout.contentRect.x, y: currentY, width: leftPaneWidth, height });
      itemRowIndexes.push(index);
    }
    currentY += height;
  }

  return {
    ...baseLayout,
    itemRects,
    itemRowIndexes,
    detailRect,
  };
}

function renderOverlayShell(buf: Buffer, layout: ModalLayout, opts: ModalOptions): void {
  const surfaceBg = PALETTE.overlaySurfaceBg;
  const titleY = layout.y + 1;
  const titleText = truncate(opts.title, Math.max(0, layout.width - 10));
  const titleX = opts.renderSpec.chrome.titleAlign === "center"
    ? layout.x + Math.max(2, Math.floor((layout.width - titleText.length) / 2))
    : layout.x + 2;

  buf.fillRect(layout, " ", { bg: surfaceBg, fg: PALETTE.default, bold: false, dim: false });
  buf.drawBorder(layout, { fg: PALETTE.brightWhite, bg: surfaceBg, bold: false, dim: false });

  buf.writeText(titleY, titleX, titleText, {
    fg: opts.renderSpec.chrome.titleColor,
    bg: surfaceBg,
    bold: true,
    dim: false,
  });

  const escapeText = opts.renderSpec.chrome.escapeText;
  const escapeX = layout.x + layout.width - escapeText.length - 2;
  if (escapeX > layout.x + 2) {
    buf.writeText(titleY, escapeX, escapeText, {
      fg: PALETTE.overlayHint,
      bg: surfaceBg,
      dim: false,
    });
  }

  if (opts.mode !== "palette" && opts.eyebrow) {
    buf.writeText(layout.y + 2, layout.x + 2, truncate(opts.eyebrow, layout.width - 4), {
      fg: PALETTE.overlayHint,
      bg: surfaceBg,
      dim: false,
    });
  }

  if (opts.footer && layout.footerRect) {
    buf.writeText(layout.footerRect.y, layout.footerRect.x, truncate(opts.footer, layout.footerRect.width), {
      fg: PALETTE.overlayHint,
      bg: surfaceBg,
    });
  }
}

function renderSingleBody(
  buf: Buffer,
  layout: ModalLayout,
  opts: MenuModalOptions | InfoModalOptions | PaletteModalOptions,
): void {
  const rows = normalizeRows(opts);
  const compactRows = shouldUseCompactRows(
    layout.height,
    getHeaderHeight(opts),
    opts.footer ? 2 : 1,
    rows,
  );
  const contentWidth = layout.contentRect.width;
  const isPalette = opts.renderSpec.family === "palette";

  if (isPalette) {
    renderQueryRow(buf, layout, opts.mode === "palette" ? opts.query : "");
  }

  let rowY = layout.contentRect.y;
  if (isPalette && rows.length === 0) {
    const emptyLabel = truncate(opts.mode === "palette" ? (opts.emptyLabel ?? "No commands match") : "No commands match", contentWidth);
    buf.writeText(rowY, layout.x + 3, emptyLabel, {
      fg: opts.renderSpec.text.mutedColor,
      bg: PALETTE.overlaySurfaceBg,
      dim: false,
    });
    return;
  }

    let selectableRectIndex = 0;
    for (let index = 0; index < rows.length; index++) {
      const row = rows[index]!;
      const previous = rows[index - 1];
    if (!isPalette && !compactRows && row.section && row.section !== previous?.section) {
      if (rowY >= layout.contentRect.y + layout.contentRect.height) break;
      buf.writeText(rowY, layout.x + 2, formatOverlayText(row.section, opts.renderSpec.text.sectionCase, contentWidth), {
        fg: opts.renderSpec.text.sectionColor,
        bg: PALETTE.overlaySurfaceBg,
        bold: true,
      });
      rowY += 1;
    }

      const rowRect = opts.mode === "info"
        ? { x: layout.x + 1, y: rowY, width: layout.width - 2, height: getRowHeight(row, compactRows, opts.renderSpec.family) }
        : (layout.itemRowIndexes[selectableRectIndex] === index
          ? layout.itemRects[selectableRectIndex++]
          : undefined) ?? {
          x: layout.x + 1,
          y: rowY,
          width: layout.width - 2,
          height: getRowHeight(row, compactRows, opts.renderSpec.family),
        };

    if (rowRect.y + rowRect.height > layout.contentRect.y + layout.contentRect.height) break;
    const isSelected = opts.mode !== "info" && index === opts.selectedIndex;
    renderSingleRow(buf, rowRect, row, isSelected, opts.renderSpec);
    rowY = rowRect.y + rowRect.height;
  }
}

function renderSplitBody(buf: Buffer, layout: ModalLayout, opts: SplitModalOptions): void {
  const dividerX = layout.detailRect ? layout.detailRect.x - 1 : layout.x + Math.floor(layout.width / 2);
  const leftRows = normalizeRows(opts);
  const rightRows = normalizeInfoRows(opts.detailItems);

  if (layout.detailRect) {
    for (let y = layout.contentRect.y; y < layout.contentRect.y + layout.contentRect.height; y++) {
      buf.set(y, dividerX, BOX.vertical, { fg: PALETTE.border, bg: PALETTE.overlaySurfaceBg });
    }
  }

  renderSplitLeftRows(buf, layout, leftRows, opts.selectedIndex, opts.renderSpec);
  renderSplitDetailRows(buf, layout.detailRect ?? layout.contentRect, rightRows, opts.renderSpec);
}

function renderQueryRow(buf: Buffer, layout: ModalLayout, query: string): void {
  const queryRect = {
    x: layout.x + 2,
    y: layout.y + 2,
    width: layout.width - 4,
    height: 1,
  };
  const queryText = query.length > 0 ? `${query}\u2588` : "\u2588";

  buf.writeText(queryRect.y, queryRect.x + 1, ">", {
    fg: PALETTE.brightWhite,
    bg: PALETTE.overlaySurfaceBg,
    bold: true,
    dim: false,
  });
  buf.writeText(queryRect.y, queryRect.x + 3, truncate(queryText, queryRect.width - 4), {
    fg: PALETTE.brightWhite,
    bg: PALETTE.overlaySurfaceBg,
    bold: true,
    dim: false,
  });
}

function renderSingleRow(
  buf: Buffer,
  rect: Rect,
  row: NormalizedModalRow,
  isSelected: boolean,
  spec: OverlayRenderSpec,
): void {
  if (spec.family === "palette") {
    renderPaletteRow(buf, rect, row, isSelected, spec);
    return;
  }

  const bg = isSelected ? spec.selection.bg : PALETTE.overlaySurfaceBg;
  const fg = isSelected ? spec.selection.fg : getRowColor(row.tone, spec, "label");
  const detailFg = isSelected ? spec.selection.fg : getRowColor(row.tone, spec, "detail");
  const hintFg = isSelected ? spec.selection.fg : spec.text.hintColor;

  buf.fillRect(rect, " ", { bg, fg: PALETTE.default, bold: false, dim: false });

  const labelX = rect.x + 2;
  const lineWidth = Math.max(0, rect.width - 4);
  const hintWidth = row.hint?.length ?? 0;
  const hintX = hintWidth > 0 ? rect.x + rect.width - hintWidth - 2 : undefined;
  const labelMax = hintX ? Math.max(0, hintX - labelX - 1) : lineWidth;
  const rawLabel = applyOverlayCase(row.label, spec.text.rowCase);
  const labelText = row.style === "block"
    ? truncatePathTail(rawLabel, labelMax)
    : truncate(rawLabel, labelMax);

  buf.writeText(rect.y, labelX, labelText, {
    fg,
    bg,
    bold: isSelected || row.tone === "accent" || row.style === "block",
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

function renderPaletteRow(
  buf: Buffer,
  rect: Rect,
  row: NormalizedModalRow,
  isSelected: boolean,
  spec: OverlayRenderSpec,
): void {
  const sectionWidth = 12;
  const gap = 2;
  const leftPad = 2;
  const hintWidth = row.hint?.length ?? 0;
  const hintX = hintWidth > 0 ? rect.x + rect.width - hintWidth - 2 : undefined;
  const sectionX = rect.x + leftPad;
  const labelX = sectionX + sectionWidth + gap;
  const labelMax = Math.max(0, (hintX ?? (rect.x + rect.width - 2)) - labelX - 1);
  const bg = isSelected ? spec.selection.bg : PALETTE.overlaySurfaceBg;
  const fg = isSelected ? spec.selection.fg : spec.text.primaryColor;

  buf.fillRect(rect, " ", { bg, fg: PALETTE.default, bold: false, dim: false });

  if (row.section) {
    buf.writeText(rect.y, sectionX, formatOverlayText(row.section, spec.text.sectionCase, sectionWidth), {
      fg: isSelected ? spec.selection.fg : spec.text.sectionColor,
      bg,
      bold: isSelected,
      dim: false,
    });
  }

  buf.writeText(rect.y, labelX, formatOverlayText(row.label, spec.text.rowCase, labelMax), {
    fg,
    bg,
    bold: true,
    dim: false,
  });

  if (row.hint && hintX && hintX > labelX + 2) {
    buf.writeText(rect.y, hintX, row.hint, {
      fg: isSelected ? spec.selection.fg : spec.text.hintColor,
      bg,
      bold: true,
      dim: false,
    });
  }
}

function renderSplitLeftRows(
  buf: Buffer,
  layout: ModalLayout,
  rows: readonly NormalizedModalRow[],
  selectedIndex: number,
  spec: OverlayRenderSpec,
): void {
  const maxWidth = Math.max(0, (layout.detailRect?.x ?? (layout.x + layout.width - 2)) - layout.contentRect.x - 2);
  let rowY = layout.contentRect.y;
  let selectableIndex = 0;

  for (let index = 0; index < rows.length; index++) {
    const row = rows[index]!;
    const previous = rows[index - 1];
    if (row.section && row.section !== previous?.section) {
      if (rowY >= layout.contentRect.y + layout.contentRect.height) break;
      buf.writeText(rowY, layout.contentRect.x, formatOverlayText(row.section, spec.text.sectionCase, maxWidth), {
        fg: spec.text.sectionColor,
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
    const bg = isSelected ? spec.selection.bg : PALETTE.overlaySurfaceBg;
    const fg = isSelected ? spec.selection.fg : getRowColor(row.tone, spec, "label");
    const label = formatOverlayText(row.label, spec.text.rowCase, Math.max(0, rect.width - 2));

    if (row.selectable) {
      buf.fillRect(rect, " ", { bg, fg: PALETTE.default, bold: false, dim: false });
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

function renderSplitDetailRows(
  buf: Buffer,
  rect: Rect,
  rows: readonly NormalizedModalRow[],
  spec: OverlayRenderSpec,
): void {
  let rowY = rect.y;
  const maxWidth = rect.width;

  for (let index = 0; index < rows.length; index++) {
    const row = rows[index]!;
    const previous = rows[index - 1];
    if (row.section && row.section !== previous?.section) {
      if (rowY >= rect.y + rect.height) break;
      buf.writeText(rowY, rect.x + 2, formatOverlayText(row.section, spec.text.sectionCase, maxWidth - 2), {
        fg: spec.text.sectionColor,
        bg: PALETTE.overlaySurfaceBg,
        bold: true,
      });
      rowY += 1;
    }
    if (rowY >= rect.y + rect.height) break;

    const text = row.detail
      ? `${truncate(formatOverlayText(row.label, spec.text.rowCase, Math.max(0, Math.floor(maxWidth * 0.34) - 2)), Math.max(0, Math.floor(maxWidth * 0.34) - 2))} ${truncate(row.detail, Math.max(0, maxWidth - Math.floor(maxWidth * 0.34) - 5))}`
      : formatOverlayText(row.label, spec.text.rowCase, maxWidth - 2);
    buf.writeText(rowY, rect.x + 2, text, {
      fg: getRowColor(row.tone, spec, row.detail ? "detail" : "label"),
      bg: PALETTE.overlaySurfaceBg,
      bold: row.tone === "accent" || row.style === "block",
    });
    rowY += 1;
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

function getHeaderHeight(opts: ModalOptions): number {
  return opts.mode === "palette" || opts.eyebrow ? 4 : 3;
}

function getRowHeight(row: NormalizedModalRow, compact = false, family?: OverlayFamily): number {
  if (family === "palette" || compact) return 1;
  if (row.detail) return 2;
  if (row.style === "block") return 2;
  return 1;
}

function getRowColor(
  tone: ModalTone,
  spec: OverlayRenderSpec,
  part: "label" | "detail",
): number {
  if (tone === "accent") return PALETTE.brightWhite;
  if (tone === "muted") return spec.text.mutedColor;
  return part === "detail" ? spec.text.detailColor : spec.text.primaryColor;
}

function dimColor(color: number): number {
  switch (color) {
    case PALETTE.default:
    case PALETTE.brightWhite:
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
    case PALETTE.gray:
      return PALETTE.dimGray;
    default:
      return PALETTE.dimGray;
  }
}

function resolveOverlayFrame(
  parent: Rect,
  spec: OverlayLayoutSpec,
  headerHeight: number,
  footerHeight: number,
  contentHeight: number,
  maxLineLength: number,
): Omit<ModalLayout, "itemRects" | "itemRowIndexes" | "detailRect"> {
  const maxWidth = Math.max(28, parent.width - 4);
  const minWidth = Math.min(spec.minWidth, maxWidth);
  const modalWidth = Math.min(
    Math.max(maxLineLength + 6, spec.preferredWidth, minWidth),
    maxWidth,
  );

  const desiredHeight = Math.max(headerHeight + contentHeight + footerHeight, spec.preferredHeight);
  const minHeight = Math.min(spec.minHeight, parent.height);
  const modalHeight = Math.min(Math.max(desiredHeight, minHeight), parent.height);

  const x = parent.x + Math.floor((parent.width - modalWidth) / 2);
  const y = parent.y + Math.floor((parent.height - modalHeight) / 2);
  const contentStartY = y + headerHeight;
  const footerRect = footerHeight > 1
    ? { x: x + 2, y: y + modalHeight - 2, width: modalWidth - 4, height: 1 }
    : undefined;
  const contentRect: Rect = {
    x: x + 2,
    y: contentStartY,
    width: modalWidth - 4,
    height: Math.max(0, (footerRect?.y ?? (y + modalHeight - 1)) - contentStartY),
  };

  return {
    x,
    y,
    width: modalWidth,
    height: modalHeight,
    contentRect,
    footerRect,
  };
}

function getSplitPaneWidths(contentWidth: number, ratio: readonly [number, number]) {
  const usableWidth = Math.max(1, contentWidth - 1);
  const minPaneWidth = Math.max(8, Math.min(20, Math.floor(usableWidth / 3)));
  const leftTarget = Math.floor((usableWidth * ratio[0]) / (ratio[0] + ratio[1]));
  const leftPaneWidth = Math.max(minPaneWidth, Math.min(leftTarget, usableWidth - minPaneWidth));
  const rightPaneWidth = usableWidth - leftPaneWidth;
  return { leftPaneWidth, rightPaneWidth };
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

function getPaletteRowLength(row: NormalizedModalRow): number {
  return (row.section?.length ?? 0) + row.label.length + (row.hint?.length ?? 0) + 12;
}

function getPaneContentHeight(rows: readonly NormalizedModalRow[]): number {
  return rows.reduce((height, row, index) => {
    const sectionHeight = row.section && row.section !== rows[index - 1]?.section ? 1 : 0;
    return height + sectionHeight + getRowHeight(row, false, "info");
  }, 0);
}

function formatOverlayText(text: string, textCase: OverlayTextCase, maxLen: number): string {
  return truncate(applyOverlayCase(text, textCase), maxLen);
}

function applyOverlayCase(text: string, textCase: OverlayTextCase): string {
  return textCase === "lower" ? text.toLowerCase() : text;
}
