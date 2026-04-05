import { PALETTE } from "../theme.js";

type ModalTone = "default" | "muted" | "accent";
type ModalStyle = "plain" | "block";

export type OverlaySizePreset = "standard" | "wide";
export type OverlayFamily = "palette" | "menu" | "split" | "info";
export type OverlayTextCase = "preserve" | "lower";
export type OverlayTitleAlign = "left" | "center";

export interface Rect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface OverlayChromeSpec {
  readonly titleAlign: OverlayTitleAlign;
  readonly titleColor: number;
  readonly escapeText: string;
}

export interface OverlaySelectionSpec {
  readonly bg: number;
  readonly fg: number;
  readonly fullWidth: boolean;
}

export interface OverlayTextSpec {
  readonly rowCase: OverlayTextCase;
  readonly sectionCase: OverlayTextCase;
  readonly primaryColor: number;
  readonly detailColor: number;
  readonly hintColor: number;
  readonly sectionColor: number;
  readonly mutedColor: number;
}

export interface OverlayLayoutSpec {
  readonly preferredWidth: number;
  readonly minWidth: number;
  readonly preferredHeight: number;
  readonly minHeight: number;
  readonly splitRatio?: readonly [number, number];
}

export interface OverlayRenderSpec {
  readonly family: OverlayFamily;
  readonly sizePreset: OverlaySizePreset;
  readonly chrome: OverlayChromeSpec;
  readonly selection: OverlaySelectionSpec;
  readonly text: OverlayTextSpec;
  readonly layout: OverlayLayoutSpec;
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
  readonly label?: string;
  readonly text?: string;
  readonly detail?: string;
  readonly hint?: string;
  readonly section?: string;
  readonly tone?: ModalTone;
  readonly style?: ModalStyle;
}

export interface ModalInfoItem extends ModalRow {
  readonly text: string;
}

export interface MenuModalOptions {
  readonly mode: "menu";
  readonly title: string;
  readonly eyebrow?: string;
  readonly items: readonly (string | ModalRow)[];
  readonly selectedIndex: number;
  readonly footer?: string;
  readonly returnTarget?: "command-palette";
  readonly renderSpec: OverlayRenderSpec;
}

export interface InfoModalOptions {
  readonly mode: "info";
  readonly title: string;
  readonly eyebrow?: string;
  readonly items: readonly ModalInfoItem[];
  readonly footer?: string;
  readonly returnTarget?: "command-palette";
  readonly renderSpec: OverlayRenderSpec;
}

export interface PaletteModalOptions {
  readonly mode: "palette";
  readonly title: string;
  readonly query: string;
  readonly items: readonly ModalRow[];
  readonly selectedIndex: number;
  readonly footer?: string;
  readonly emptyLabel?: string;
  readonly renderSpec: OverlayRenderSpec;
}

export interface SplitModalRow extends ModalRow {
  readonly selectable?: boolean;
}

export interface SplitModalOptions {
  readonly mode: "split";
  readonly title: string;
  readonly eyebrow?: string;
  readonly items: readonly SplitModalRow[];
  readonly selectedIndex: number;
  readonly detailItems: readonly ModalInfoItem[];
  readonly footer?: string;
  readonly emptyLabel?: string;
  readonly returnTarget?: "command-palette";
  readonly renderSpec: OverlayRenderSpec;
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
  readonly label: string;
  readonly detail?: string;
  readonly hint?: string;
  readonly section?: string;
  readonly tone: ModalTone;
  readonly style: ModalStyle;
  readonly selectable: boolean;
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
        family: "split",
        sizePreset: "wide",
        chrome: STANDARD_CHROME,
        selection: STANDARD_SELECTION,
        text: STANDARD_TEXT,
        layout: { ...WIDE_LAYOUT, splitRatio: [46, 54] },
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

export function layoutModal(parent: Rect, opts: ModalOptions): ModalLayout {
  return opts.mode === "split"
    ? layoutSplitModal(parent, opts)
    : layoutSingleModal(parent, opts);
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
      : Math.max(0, ...getEyebrowLines("eyebrow" in opts ? opts.eyebrow : undefined).map((line) => line.length)),
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
    Math.max(0, ...getEyebrowLines(opts.eyebrow).map((line) => line.length)),
    opts.footer?.length ?? 0,
    ...leftRows.flatMap((row) => [
      (row.section?.length ?? 0) + 4,
      row.label.length + (row.detail?.length ?? 0) + (row.hint?.length ?? 0) + 8,
    ]),
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
  if (opts.mode === "palette") return 4;
  const eyebrowLines = getEyebrowLines("eyebrow" in opts ? opts.eyebrow : undefined);
  return eyebrowLines.length > 0 ? 3 + eyebrowLines.length : 3;
}

function getRowHeight(row: NormalizedModalRow, compact = false, family?: OverlayFamily): number {
  if (family === "palette" || compact) return 1;
  if (row.detail) return 2;
  if (row.style === "block") return 2;
  return 1;
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

function getEyebrowLines(eyebrow?: string): readonly string[] {
  if (!eyebrow) return [];
  return eyebrow.split("\n");
}
