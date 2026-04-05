import { TextAttributes, type MouseEvent } from "@opentui/core";
import { sanitizeTerminalText } from "../../../lib/sanitize.js";

import type { AppState } from "../../state/reducer.js";
import { truncate } from "../../format.js";
import type {
  InfoModalOptions,
  MenuModalOptions,
  ModalInfoItem,
  ModalLayout,
  ModalOptions,
  OverlayTextCase,
  ModalRow,
  PaletteModalOptions,
  SplitModalOptions,
  SplitModalRow,
} from "../../shared/modal-model.js";
import { layoutModal } from "../../shared/modal-model.js";
  import {
    OPEN_TUI_THEME,
    buildFeatureListLines,
    buildFocusLines,
    buildFooterModel,
  buildHeaderModel,
  buildLogLines,
  buildModalModel,
  buildSessionLines,
    buildStatusStripModel,
    computeScreenLayout,
    getModalParentRect,
    resolveMissionControlTheme,
    type UiLine,
  } from "./builders.js";

export interface MissionControlScreenProps {
  readonly state: AppState;
  readonly width: number;
  readonly height: number;
  readonly animationFrame?: number;
  readonly elapsedOffsetMs?: number;
  readonly onMouseDown?: (event: MouseEvent) => void;
}

export function MissionControlScreen({
  state,
  width,
  height,
  animationFrame = 0,
  elapsedOffsetMs = 0,
  onMouseDown,
  }: MissionControlScreenProps) {
    const layout = computeScreenLayout(width, height, state.snapshot);
    const theme = resolveMissionControlTheme(state.snapshot);
    const header = buildHeaderModel(state.snapshot, animationFrame);
    const status = buildStatusStripModel(state.snapshot);
  const footer = buildFooterModel(state.snapshot, state.copyMode);

  if (width < 80 || height < 24) {
    return (
      <box
        width={width}
          height={height}
          border
          flexDirection="column"
          backgroundColor={theme.pageBg}
          >
        <box paddingLeft={1} paddingRight={1} paddingTop={1} flexDirection="column">
          <SafeText fg={OPEN_TUI_THEME.accent} attributes={TextAttributes.BOLD}>Mission Control</SafeText>
          <SafeText fg={OPEN_TUI_THEME.text} attributes={TextAttributes.BOLD}>Terminal too small</SafeText>
          <SafeText fg={OPEN_TUI_THEME.muted}>
            Resize to at least 80x24 for the interactive dashboard, or use --size for deterministic previews.
          </SafeText>
          <SafeText fg={OPEN_TUI_THEME.muted}>{`Current: ${width}x${height}`}</SafeText>
        </box>
      </box>
    );
  }

  const focusLines = buildFocusLines(state, contentWidth(layout.mainWidth), contentHeight(layout.leftTopHeight));
  const featureLines = buildFeatureListLines(state, contentWidth(layout.sideWidth), contentHeight(layout.rightTopHeight));
  const logLines = buildLogLines(state, contentWidth(layout.mainWidth), contentHeight(layout.leftBottomHeight));
  const sessionLines = buildSessionLines(state, contentWidth(layout.sideWidth), contentHeight(layout.rightBottomHeight), elapsedOffsetMs);
  const modal = buildModalModel(state);
  const backdropActive = modal !== undefined;
  const modalParentRect = getModalParentRect(layout);
  const modalLayout = modal ? layoutModal(modalParentRect, modal) : undefined;

  return (
      <box
        width={width}
        height={height}
        border
        borderColor={backdropActive ? dimHexColor(OPEN_TUI_THEME.text) : undefined}
        flexDirection="column"
        backgroundColor={theme.pageBg}
        onMouseDown={onMouseDown}
      >
      <box width="100%" height={1} flexDirection="row" justifyContent="space-between">
        <SafeText fg={backdropColor(header.left.fg, backdropActive)} attributes={mergeTextAttributes(header.left.attributes, backdropActive)}>{header.left.text}</SafeText>
        <SafeText fg={backdropColor(header.right.fg, backdropActive)} attributes={mergeTextAttributes(undefined, backdropActive)}>{header.right.text}</SafeText>
      </box>

      <box width="100%" height={2} flexDirection="column">
        <box flexDirection="row" justifyContent="space-between">
          <SafeText fg={backdropColor(status.primaryLeft.fg, backdropActive)} attributes={mergeTextAttributes(status.primaryLeft.attributes, backdropActive)}>{status.primaryLeft.text}</SafeText>
          {status.primaryRight ? <SafeText fg={backdropColor(status.primaryRight.fg, backdropActive)} attributes={mergeTextAttributes(status.primaryRight.attributes, backdropActive)}>{status.primaryRight.text}</SafeText> : <box />}
        </box>
        <box flexDirection="row" justifyContent="space-between">
          {status.secondaryLeft ? <SafeText fg={backdropColor(status.secondaryLeft.fg, backdropActive)} attributes={mergeTextAttributes(status.secondaryLeft.attributes, backdropActive)}>{status.secondaryLeft.text}</SafeText> : <box />}
          {status.secondaryRight ? <SafeText fg={backdropColor(status.secondaryRight.fg, backdropActive)} attributes={mergeTextAttributes(status.secondaryRight.attributes, backdropActive)}>{status.secondaryRight.text}</SafeText> : <box />}
        </box>
      </box>

      {layout.stacked ? (
        <StackedBody
          layout={layout}
          focusLines={focusLines}
          featureLines={featureLines}
            logLines={logLines}
            sessionLines={sessionLines}
            state={state}
            theme={theme}
            dimmed={backdropActive}
          />
        ) : (
          <SplitBody
          layout={layout}
          focusLines={focusLines}
          featureLines={featureLines}
            logLines={logLines}
            sessionLines={sessionLines}
            state={state}
            theme={theme}
            dimmed={backdropActive}
          />
        )}

          <box width="100%" height={1} flexDirection="row" justifyContent="space-between" backgroundColor={backdropColor(theme.headerBg, backdropActive)}>
            <SafeText fg={backdropColor(state.copyMode ? theme.warning : theme.muted, backdropActive)} attributes={mergeTextAttributes(state.copyMode ? TextAttributes.BOLD : undefined, backdropActive)}>
              {footer.left}
            </SafeText>
            <SafeText fg={backdropColor(theme.muted, backdropActive)} attributes={mergeTextAttributes(undefined, backdropActive)}>{footer.right}</SafeText>
          </box>

      {modal ? (
        <ModalLayer
            modal={modal}
            state={state}
            layout={modalLayout!}
            theme={theme}
          />
        ) : null}
        </box>
  );
}

interface BodyProps {
  readonly layout: ReturnType<typeof computeScreenLayout>;
  readonly focusLines: readonly UiLine[];
  readonly featureLines: readonly UiLine[];
  readonly logLines: readonly UiLine[];
  readonly sessionLines: readonly UiLine[];
  readonly state: AppState;
  readonly theme: ReturnType<typeof resolveMissionControlTheme>;
  readonly dimmed: boolean;
}

function SplitBody({ layout, focusLines, featureLines, logLines, sessionLines, state, theme, dimmed }: BodyProps) {
  return (
    <box width="100%" height={layout.bodyHeight} flexDirection="row">
      <box width={layout.mainWidth} height={layout.bodyHeight} flexDirection="column" paddingRight={1}>
          <PanelFrame title={state.snapshot.mode === "home" ? "Overview" : state.leftPaneMode === "overview" ? "Mission Overview" : "Focus / Preview"} height={layout.leftTopHeight} theme={theme} dimmed={dimmed}>
            <LineList lines={focusLines} dimmed={dimmed} />
          </PanelFrame>
        <Spacer />
          <PanelFrame title={state.snapshot.mode === "home" ? "Pending Handoffs" : "Timeline"} height={layout.leftBottomHeight} theme={theme} dimmed={dimmed}>
            <LineList lines={logLines} dimmed={dimmed} />
          </PanelFrame>
      </box>

      <box width={layout.sideWidth} height={layout.bodyHeight} flexDirection="column">
          <PanelFrame title={state.snapshot.mode === "home" ? "Environment" : "Tasks"} height={layout.rightTopHeight} theme={theme} dimmed={dimmed}>
            <LineList lines={featureLines} dimmed={dimmed} />
          </PanelFrame>
        <Spacer />
          <PanelFrame title="Activity / Session" height={layout.rightBottomHeight} theme={theme} dimmed={dimmed}>
            <LineList lines={sessionLines} dimmed={dimmed} />
          </PanelFrame>
      </box>
    </box>
  );
}

function StackedBody({ layout, focusLines, featureLines, logLines, sessionLines, state, theme, dimmed }: BodyProps) {
  const [focusHeight, listHeight, logHeight, sessionHeight] = layout.stackedHeights;
  return (
    <box width="100%" height={layout.bodyHeight} flexDirection="column">
      <PanelFrame title={state.snapshot.mode === "home" ? "Overview" : state.leftPaneMode === "overview" ? "Mission Overview" : "Focus / Preview"} height={focusHeight} theme={theme} dimmed={dimmed}>
        <LineList lines={focusLines} dimmed={dimmed} />
      </PanelFrame>
      <Spacer />
      <PanelFrame title={state.snapshot.mode === "home" ? "Environment" : "Tasks"} height={listHeight} theme={theme} dimmed={dimmed}>
        <LineList lines={featureLines} dimmed={dimmed} />
      </PanelFrame>
      <Spacer />
      <PanelFrame title={state.snapshot.mode === "home" ? "Pending Handoffs" : "Timeline"} height={logHeight} theme={theme} dimmed={dimmed}>
        <LineList lines={logLines} dimmed={dimmed} />
      </PanelFrame>
      <Spacer />
      <PanelFrame title="Activity / Session" height={sessionHeight} theme={theme} dimmed={dimmed}>
        <LineList lines={sessionLines} dimmed={dimmed} />
      </PanelFrame>
    </box>
  );
}

interface PanelFrameProps {
  readonly title: string;
  readonly height: number;
  readonly theme: ReturnType<typeof resolveMissionControlTheme>;
  readonly dimmed: boolean;
  readonly children: React.ReactNode;
}

function PanelFrame({ title, height, theme, dimmed, children }: PanelFrameProps) {
  return (
    <box
      title={title}
      border
      borderColor={dimmed ? dimHexColor(OPEN_TUI_THEME.text) : undefined}
      width="100%"
      height={Math.max(3, height)}
      flexDirection="column"
      backgroundColor={theme.panelBg}
      paddingLeft={1}
      paddingRight={1}
    >
      {children}
    </box>
  );
}

function Spacer() {
  return <box height={1} />;
}

interface LineListProps {
  readonly lines: readonly UiLine[];
  readonly dimmed: boolean;
}

function LineList({ lines, dimmed }: LineListProps) {
  return (
    <box flexDirection="column" width="100%" height="100%">
      {lines.map((line, index) => (
        <box key={index} width="100%" height={1} backgroundColor={backdropColor(line.bg, dimmed)}>
          <SafeText fg={backdropColor(line.fg, dimmed)} attributes={mergeTextAttributes(line.attributes, dimmed)}>{line.text}</SafeText>
        </box>
      ))}
    </box>
  );
}

interface ModalLayerProps {
  readonly modal: ModalOptions;
  readonly state: AppState;
  readonly layout: ModalLayout;
  readonly theme: ReturnType<typeof resolveMissionControlTheme>;
}

function ModalLayer({ modal, state, layout, theme }: ModalLayerProps) {
  const eyebrowLines = "eyebrow" in modal && modal.eyebrow ? modal.eyebrow.split("\n") : [];
  const paletteOrigin = modal.mode === "palette" || modal.returnTarget === "command-palette";
  const modalBackgroundColor = paletteOrigin
    ? theme.paletteModalBg
    : theme.modalBg;
  const shouldFillSurface = modalBackgroundColor !== undefined
    || (paletteOrigin && modal.mode !== "palette");
  const escapeText = "esc";
  const contentWidth = Math.max(0, layout.width - 4);
  return (
    <box
      position="absolute"
      left={layout.x}
      top={layout.y}
      width={layout.width}
      height={layout.height}
      border
      flexDirection="column"
        backgroundColor={modalBackgroundColor}
        paddingLeft={1}
        paddingRight={1}
      >
          {shouldFillSurface ? <ModalSurfaceFill width={layout.width - 2} height={layout.height - 2} /> : null}
            <box width="100%" flexDirection="row" alignItems="center">
            {modal.mode === "palette" ? (
              <>
                <SafeText fg={OPEN_TUI_THEME.text} attributes={TextAttributes.BOLD}>
                  {composePaletteHeaderTitle(modal.title, escapeText, contentWidth)}
                </SafeText>
                <SafeText fg={OPEN_TUI_THEME.muted}>{composePaletteHeaderEscape(escapeText, contentWidth)}</SafeText>
              </>
            ) : (
              <>
                <box flexGrow={1}>
                <SafeText fg={OPEN_TUI_THEME.accent} attributes={TextAttributes.BOLD}>{modal.title}</SafeText>
              </box>
              <SafeText fg={OPEN_TUI_THEME.muted}>{escapeText}</SafeText>
            </>
          )}
        </box>

      {eyebrowLines.map((line, index) => (
        <SafeText key={index} fg={OPEN_TUI_THEME.muted}>{line}</SafeText>
      ))}

          {modal.mode === "split" ? (
              <SplitModalBody
                modal={modal}
                width={layout.width}
                height={layout.height}
                theme={theme}
                transparentPanels={paletteOrigin && theme.paletteModalBg === undefined}
              />
            ) : modal.mode === "info" ? (
              <InfoModalBody modal={modal} />
            ) : modal.mode === "palette" ? (
          <PaletteModalBody modal={modal} contentWidth={contentWidth} />
        ) : (
          <MenuModalBody modal={modal} />
        )}

      {modal.footer ? (
        <box marginTop={1}>
          <SafeText fg={OPEN_TUI_THEME.muted}>{modal.footer}</SafeText>
        </box>
      ) : null}
    </box>
  );
}

function MenuModalBody({ modal }: { readonly modal: MenuModalOptions }) {
  const items = modal.items.map((item) => normalizeModalRow(item));
  return (
    <box flexDirection="column" width="100%" flexGrow={1}>
      {items.length === 0 ? (
        <SafeText fg={OPEN_TUI_THEME.muted}>{modal.footer ?? "No items"}</SafeText>
      ) : items.map((item, index) => (
        <ModalRowView
          key={index}
          row={item}
          selected={index === modal.selectedIndex}
        />
      ))}
    </box>
  );
}

function PaletteModalBody({
  modal,
  contentWidth,
}: {
  readonly modal: PaletteModalOptions;
  readonly contentWidth: number;
}) {
  const items = modal.items.map((item) => normalizeModalRow(item));
  const queryText = modal.query.length > 0 ? `${modal.query}\u2588` : "\u2588";
  return (
        <box flexDirection="column" width="100%" flexGrow={1}>
        <box marginBottom={1}>
          <SafeText fg={OPEN_TUI_THEME.text} attributes={TextAttributes.BOLD}>{padLine(`> ${queryText}`, contentWidth)}</SafeText>
        </box>
        {items.length === 0 ? (
          <SafeText fg={OPEN_TUI_THEME.muted}>{padLine(modal.emptyLabel ?? "No commands match your filter", contentWidth)}</SafeText>
        ) : items.map((item, index) => (
            <PaletteModalRowView
              key={index}
              row={item}
              selected={index === modal.selectedIndex}
              width={contentWidth}
              textCase={modal.renderSpec.text.rowCase}
              sectionCase={modal.renderSpec.text.sectionCase}
            />
          ))}
        </box>
      );
}

function InfoModalBody({ modal }: { readonly modal: InfoModalOptions }) {
  return (
    <box flexDirection="column" width="100%" flexGrow={1}>
      {modal.items.map((item, index) => (
        <InfoItemView key={index} item={item} />
      ))}
    </box>
  );
}

function SplitModalBody({
  modal,
  width,
  theme,
  transparentPanels,
  }: {
      readonly modal: SplitModalOptions;
      readonly width: number;
      readonly height: number;
    readonly theme: ReturnType<typeof resolveMissionControlTheme>;
      readonly transparentPanels: boolean;
    }) {
  const ratio = modal.renderSpec.layout.splitRatio ?? [46, 54];
  const total = ratio[0] + ratio[1];
  const leftWidth = Math.max(18, Math.floor((width - 3) * ratio[0] / total));
  const rightWidth = Math.max(18, width - 3 - leftWidth);
  const items = modal.items.map((item) => normalizeSplitModalRow(item));

  return (
    <box width="100%" flexGrow={1} flexDirection="row" marginTop={1}>
            <box width={leftWidth} height="100%" border title="List" paddingLeft={1} paddingRight={1} backgroundColor={transparentPanels ? undefined : theme.modalPanelBg}>
          <box width="100%" height="100%" flexDirection="column">
            {items.length === 0 ? (
              <SafeText fg={OPEN_TUI_THEME.muted}>{modal.emptyLabel ?? "No items"}</SafeText>
            ) : items.map((item, index) => (
                  <ModalRowView
                    key={index}
                  row={item}
                  selected={index === modal.selectedIndex}
                  mode={modal.mode}
              />
            ))}
          </box>
      </box>
        <box width={1} />
            <box width={rightWidth} height="100%" border title="Detail" paddingLeft={1} paddingRight={1} backgroundColor={transparentPanels ? undefined : theme.modalPanelBg}>
          <box width="100%" height="100%" flexDirection="column">
            {modal.detailItems.length === 0 ? (
              <SafeText fg={OPEN_TUI_THEME.muted}>No details</SafeText>
            ) : modal.detailItems.map((item, index) => (
              <InfoItemView key={index} item={item} />
            ))}
        </box>
      </box>
    </box>
  );
}

function ModalRowView({
  row,
  selected,
  mode,
}: {
  readonly row: NormalizedModalRow;
  readonly selected: boolean;
  readonly mode: ModalOptions["mode"];
}) {
  const selectedFg = mode === "palette"
    ? OPEN_TUI_THEME.paletteSelectionFg
    : OPEN_TUI_THEME.selectionFg;
  const selectedBg = mode === "palette"
    ? OPEN_TUI_THEME.paletteSelectionBg
    : OPEN_TUI_THEME.selectionBg;
  const fg = selected ? selectedFg : toneColor(row.tone);
  const detail = row.detail ? `  ${row.detail}` : "";
  const hint = row.hint ? `  [${row.hint}]` : "";
  return (
    <box width="100%" backgroundColor={selected ? selectedBg : undefined}>
      <SafeText fg={fg} attributes={selected ? TextAttributes.BOLD : row.style === "block" ? TextAttributes.BOLD : undefined}>{`${row.label}${detail}${hint}`}</SafeText>
    </box>
  );
}

function PaletteModalRowView({
  row,
  selected,
  width,
  textCase,
  sectionCase,
}: {
  readonly row: NormalizedModalRow;
  readonly selected: boolean;
  readonly width: number;
  readonly textCase: OverlayTextCase;
  readonly sectionCase: OverlayTextCase;
}) {
  const selectedFg = OPEN_TUI_THEME.paletteSelectionFg;
  const selectedBg = OPEN_TUI_THEME.paletteSelectionBg;
  const line = composePaletteCommandSegments({
      section: applyTextCase(row.section ?? "", sectionCase),
      label: applyTextCase(row.label, textCase),
      hint: row.hint ? `[${row.hint}]` : "",
      width,
    });
    return (
      <box width="100%" flexDirection="row" backgroundColor={selected ? selectedBg : undefined}>
        <SafeText
          fg={selected ? selectedFg : OPEN_TUI_THEME.muted}
          attributes={selected ? TextAttributes.BOLD : undefined}
        >
          {line.section}
        </SafeText>
        <SafeText>{line.sectionGap}</SafeText>
        <SafeText fg={selected ? selectedFg : OPEN_TUI_THEME.text} attributes={TextAttributes.BOLD}>
          {line.label}
        </SafeText>
        {line.labelGap.length > 0 ? <SafeText>{line.labelGap}</SafeText> : null}
        {line.hint.length > 0 ? (
          <SafeText
            fg={selected ? selectedFg : OPEN_TUI_THEME.info}
            attributes={selected ? TextAttributes.BOLD : undefined}
          >
            {line.hint}
          </SafeText>
        ) : null}
      </box>
    );
  }

function InfoItemView({ item }: { readonly item: ModalInfoItem }) {
  const prefix = item.detail ? `${item.text}: ${item.detail}` : item.text;
  const attributes = item.style === "block" ? TextAttributes.BOLD : item.tone === "accent" ? TextAttributes.BOLD : undefined;
  return (
    <SafeText fg={toneColor(item.tone)} attributes={attributes}>{prefix}</SafeText>
  );
}

type NormalizedModalTone = ModalRow["tone"] | undefined;

interface NormalizedModalRow {
  readonly label: string;
  readonly detail?: string;
  readonly hint?: string;
  readonly section?: string;
  readonly tone?: NormalizedModalTone;
  readonly style?: ModalRow["style"];
}

function normalizeModalRow(item: string | ModalRow): NormalizedModalRow {
  if (typeof item === "string") {
    return { label: item };
  }
    return {
      label: item.label ?? item.text ?? "",
      detail: item.detail,
      hint: item.hint,
      section: item.section,
      tone: item.tone,
      style: item.style,
    };
  }

function normalizeSplitModalRow(item: SplitModalRow): NormalizedModalRow {
  return {
    label: item.label ?? item.text ?? "",
    detail: item.detail,
    hint: item.hint,
    section: item.section,
    tone: item.tone,
    style: item.style,
  };
}

function toneColor(tone: NormalizedModalTone): string {
  switch (tone) {
    case "accent":
      return OPEN_TUI_THEME.accent;
    case "muted":
      return OPEN_TUI_THEME.muted;
    default:
      return OPEN_TUI_THEME.text;
  }
}

function ModalSurfaceFill({ width, height }: { readonly width: number; readonly height: number }) {
  return (
    <box position="absolute" left={1} top={1} width={width} height={height} flexDirection="column">
      {Array.from({ length: Math.max(0, height) }, (_, index) => (
        <SafeText key={index}>{" ".repeat(Math.max(0, width))}</SafeText>
      ))}
    </box>
  );
}

function composePaletteHeaderTitle(title: string, escapeText: string, width: number): string {
  if (width <= 0) return "";
  if (width <= escapeText.length + 1) return truncate(`${title} `, width);

  const contentWidth = width - escapeText.length - 1;
  return centerText(truncate(title, contentWidth), contentWidth);
}

function composePaletteHeaderEscape(escapeText: string, width: number): string {
  if (width <= 0) return "";
  if (width <= escapeText.length + 1) return "";
  return ` ${escapeText}`;
}

function centerText(text: string, width: number): string {
  if (text.length >= width) return text;
  const left = Math.floor((width - text.length) / 2);
  const right = width - text.length - left;
  return `${" ".repeat(left)}${text}${" ".repeat(right)}`;
}

function padLine(text: string, width: number): string {
  if (width <= 0) return "";
  const clipped = truncate(text, width);
  return clipped.length >= width ? clipped : `${clipped}${" ".repeat(width - clipped.length)}`;
}

function backdropColor(color: string | undefined, dimmed: boolean): string | undefined {
  if (!dimmed || !color) return color;
  return dimHexColor(color);
}

function mergeTextAttributes(attributes: number | undefined, dimmed: boolean): number | undefined {
  if (!dimmed) return attributes;
  return (attributes ?? 0) | TextAttributes.DIM;
}

function dimHexColor(color: string): string {
  const normalized = color.trim();
  if (!normalized.startsWith("#")) return color;
  const hex = normalized.slice(1);
  const fullHex = hex.length === 3
    ? hex.split("").map((char) => `${char}${char}`).join("")
    : hex;
  if (fullHex.length !== 6) return color;

  const [r, g, b] = [0, 2, 4].map((offset) => Number.parseInt(fullHex.slice(offset, offset + 2), 16));
  const factor = 0.55;
  const toHex = (value: number) => Math.max(0, Math.min(255, Math.round(value * factor))).toString(16).padStart(2, "0");
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

function applyTextCase(text: string, mode: OverlayTextCase): string {
  return mode === "lower" ? text.toLowerCase() : text;
}

function composePaletteCommandSegments({
  section,
  label,
  hint,
  width,
}: {
  readonly section: string;
  readonly label: string;
  readonly hint: string;
  readonly width: number;
}): {
  readonly section: string;
  readonly sectionGap: string;
  readonly label: string;
  readonly labelGap: string;
  readonly hint: string;
} {
  const sectionWidth = 12;
  const gap = 2;
  const sectionPart = padLine(section, Math.min(sectionWidth, width));
  const hintWidth = hint.length > 0 ? hint.length + gap : 0;
  const labelWidth = Math.max(0, width - sectionPart.length - gap - hintWidth);
  const labelPart = padLine(truncate(label, labelWidth), labelWidth);
  return {
    section: sectionPart,
    sectionGap: " ".repeat(Math.min(gap, Math.max(0, width - sectionPart.length))),
    label: labelPart,
    labelGap: hint.length > 0 ? " ".repeat(gap) : "",
    hint,
  };
}

function contentWidth(panelWidth: number): number {
  return Math.max(8, panelWidth - 4);
}

function contentHeight(panelHeight: number): number {
  return Math.max(1, panelHeight - 2);
}

interface SafeTextProps {
  readonly children: string;
  readonly fg?: string;
  readonly attributes?: number;
}

function SafeText({ children, fg, attributes }: SafeTextProps) {
  return (
    <text fg={fg} attributes={attributes}>
      {sanitizeTerminalText(children)}
    </text>
  );
}
