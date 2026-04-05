import { TextAttributes, type MouseEvent } from "@opentui/core";
import { sanitizeTerminalText } from "../../../lib/sanitize.js";

import type { AppState } from "../../state/reducer.js";
import type {
  InfoModalOptions,
  MenuModalOptions,
  ModalInfoItem,
  ModalOptions,
  ModalRow,
  PaletteModalOptions,
  SplitModalOptions,
  SplitModalRow,
} from "../../shared/modal-model.js";
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
  const modalParentRect = getModalParentRect(layout);

  return (
    <box
      width={width}
      height={height}
      border
      flexDirection="column"
      backgroundColor={theme.pageBg}
      onMouseDown={onMouseDown}
    >
      <box width="100%" height={1} flexDirection="row" justifyContent="space-between">
        <SafeText fg={header.left.fg} attributes={header.left.attributes}>{header.left.text}</SafeText>
        <SafeText fg={header.right.fg}>{header.right.text}</SafeText>
      </box>

      <box width="100%" height={2} flexDirection="column">
        <box flexDirection="row" justifyContent="space-between">
          <SafeText fg={status.primaryLeft.fg} attributes={status.primaryLeft.attributes}>{status.primaryLeft.text}</SafeText>
          {status.primaryRight ? <SafeText fg={status.primaryRight.fg} attributes={status.primaryRight.attributes}>{status.primaryRight.text}</SafeText> : <box />}
        </box>
        <box flexDirection="row" justifyContent="space-between">
          {status.secondaryLeft ? <SafeText fg={status.secondaryLeft.fg} attributes={status.secondaryLeft.attributes}>{status.secondaryLeft.text}</SafeText> : <box />}
          {status.secondaryRight ? <SafeText fg={status.secondaryRight.fg} attributes={status.secondaryRight.attributes}>{status.secondaryRight.text}</SafeText> : <box />}
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
        />
      )}

        <box width="100%" height={1} flexDirection="row" justifyContent="space-between" backgroundColor={theme.headerBg}>
          <SafeText fg={state.copyMode ? theme.warning : theme.muted} attributes={state.copyMode ? TextAttributes.BOLD : undefined}>
            {footer.left}
          </SafeText>
          <SafeText fg={theme.muted}>{footer.right}</SafeText>
        </box>

      {modal ? (
        <ModalLayer
          modal={modal}
          state={state}
            width={modalParentRect.width}
            height={modalParentRect.height}
            left={modalParentRect.x}
            top={modalParentRect.y}
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
}

function SplitBody({ layout, focusLines, featureLines, logLines, sessionLines, state, theme }: BodyProps) {
  return (
    <box width="100%" height={layout.bodyHeight} flexDirection="row">
      <box width={layout.mainWidth} height={layout.bodyHeight} flexDirection="column" paddingRight={1}>
          <PanelFrame title={state.snapshot.mode === "home" ? "Overview" : state.leftPaneMode === "overview" ? "Mission Overview" : "Focus / Preview"} height={layout.leftTopHeight} theme={theme}>
            <LineList lines={focusLines} />
          </PanelFrame>
        <Spacer />
          <PanelFrame title={state.snapshot.mode === "home" ? "Pending Handoffs" : "Timeline"} height={layout.leftBottomHeight} theme={theme}>
            <LineList lines={logLines} />
          </PanelFrame>
      </box>

      <box width={layout.sideWidth} height={layout.bodyHeight} flexDirection="column">
          <PanelFrame title={state.snapshot.mode === "home" ? "Environment" : "Tasks"} height={layout.rightTopHeight} theme={theme}>
            <LineList lines={featureLines} />
          </PanelFrame>
        <Spacer />
          <PanelFrame title="Activity / Session" height={layout.rightBottomHeight} theme={theme}>
            <LineList lines={sessionLines} />
          </PanelFrame>
      </box>
    </box>
  );
}

function StackedBody({ layout, focusLines, featureLines, logLines, sessionLines, state, theme }: BodyProps) {
  const [focusHeight, listHeight, logHeight, sessionHeight] = layout.stackedHeights;
  return (
    <box width="100%" height={layout.bodyHeight} flexDirection="column">
      <PanelFrame title={state.snapshot.mode === "home" ? "Overview" : state.leftPaneMode === "overview" ? "Mission Overview" : "Focus / Preview"} height={focusHeight} theme={theme}>
        <LineList lines={focusLines} />
      </PanelFrame>
      <Spacer />
      <PanelFrame title={state.snapshot.mode === "home" ? "Environment" : "Tasks"} height={listHeight} theme={theme}>
        <LineList lines={featureLines} />
      </PanelFrame>
      <Spacer />
      <PanelFrame title={state.snapshot.mode === "home" ? "Pending Handoffs" : "Timeline"} height={logHeight} theme={theme}>
        <LineList lines={logLines} />
      </PanelFrame>
      <Spacer />
      <PanelFrame title="Activity / Session" height={sessionHeight} theme={theme}>
        <LineList lines={sessionLines} />
      </PanelFrame>
    </box>
  );
}

interface PanelFrameProps {
  readonly title: string;
  readonly height: number;
  readonly theme: ReturnType<typeof resolveMissionControlTheme>;
  readonly children: React.ReactNode;
}

function PanelFrame({ title, height, theme, children }: PanelFrameProps) {
  return (
    <box
      title={title}
      border
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
}

function LineList({ lines }: LineListProps) {
  return (
    <box flexDirection="column" width="100%" height="100%">
      {lines.map((line, index) => (
        <box key={index} width="100%" height={1} backgroundColor={line.bg}>
          <SafeText fg={line.fg} attributes={line.attributes}>{line.text}</SafeText>
        </box>
      ))}
    </box>
  );
}

interface ModalLayerProps {
  readonly modal: ModalOptions;
  readonly state: AppState;
  readonly width: number;
  readonly height: number;
  readonly left: number;
  readonly top: number;
  readonly theme: ReturnType<typeof resolveMissionControlTheme>;
}

function ModalLayer({ modal, state, width, height, left, top, theme }: ModalLayerProps) {
  const eyebrowLines = "eyebrow" in modal && modal.eyebrow ? modal.eyebrow.split("\n") : [];
  return (
    <box
      position="absolute"
      left={left}
      top={top}
        width={width}
        height={height}
        border
        flexDirection="column"
        backgroundColor={theme.modalBg}
        paddingLeft={1}
        paddingRight={1}
      >
      <box width="100%" flexDirection="row" justifyContent="space-between">
        <SafeText fg={OPEN_TUI_THEME.accent} attributes={TextAttributes.BOLD}>{modal.title}</SafeText>
        <SafeText fg={OPEN_TUI_THEME.muted}>esc</SafeText>
      </box>

      {eyebrowLines.map((line, index) => (
        <SafeText key={index} fg={OPEN_TUI_THEME.muted}>{line}</SafeText>
      ))}

      {modal.mode === "split" ? (
          <SplitModalBody modal={modal} width={width} height={height} theme={theme} />
        ) : modal.mode === "info" ? (
          <InfoModalBody modal={modal} />
        ) : modal.mode === "palette" ? (
        <PaletteModalBody modal={modal} />
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

function PaletteModalBody({ modal }: { readonly modal: PaletteModalOptions }) {
  const items = modal.items.map((item) => normalizeModalRow(item));
  return (
    <box flexDirection="column" width="100%" flexGrow={1}>
      <box marginTop={1} marginBottom={1}>
        <SafeText fg={OPEN_TUI_THEME.warning}>{`/ ${modal.query.length > 0 ? modal.query : "type to filter"}`}</SafeText>
      </box>
      {items.length === 0 ? (
        <SafeText fg={OPEN_TUI_THEME.muted}>{modal.emptyLabel ?? "No commands match your filter"}</SafeText>
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
  }: {
      readonly modal: SplitModalOptions;
      readonly width: number;
      readonly height: number;
    readonly theme: ReturnType<typeof resolveMissionControlTheme>;
  }) {
  const ratio = modal.renderSpec.layout.splitRatio ?? [46, 54];
  const total = ratio[0] + ratio[1];
  const leftWidth = Math.max(18, Math.floor((width - 3) * ratio[0] / total));
  const rightWidth = Math.max(18, width - 3 - leftWidth);
  const items = modal.items.map((item) => normalizeSplitModalRow(item));

  return (
    <box width="100%" flexGrow={1} flexDirection="row" marginTop={1}>
          <box width={leftWidth} height="100%" border title="List" paddingLeft={1} paddingRight={1} backgroundColor={theme.modalPanelBg}>
          <box width="100%" height="100%" flexDirection="column">
            {items.length === 0 ? (
              <SafeText fg={OPEN_TUI_THEME.muted}>{modal.emptyLabel ?? "No items"}</SafeText>
            ) : items.map((item, index) => (
              <ModalRowView
                key={index}
              row={item}
              selected={index === modal.selectedIndex}
            />
          ))}
        </box>
      </box>
        <box width={1} />
          <box width={rightWidth} height="100%" border title="Detail" paddingLeft={1} paddingRight={1} backgroundColor={theme.modalPanelBg}>
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
}: {
  readonly row: NormalizedModalRow;
  readonly selected: boolean;
}) {
  const fg = selected ? OPEN_TUI_THEME.selectionFg : toneColor(row.tone);
  const detail = row.detail ? `  ${row.detail}` : "";
  const hint = row.hint ? `  [${row.hint}]` : "";
  return (
    <box width="100%" backgroundColor={selected ? OPEN_TUI_THEME.selectionBg : undefined}>
      <SafeText fg={fg} attributes={selected ? TextAttributes.BOLD : row.style === "block" ? TextAttributes.BOLD : undefined}>{`${row.label}${detail}${hint}`}</SafeText>
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
    tone: item.tone,
    style: item.style,
  };
}

function normalizeSplitModalRow(item: SplitModalRow): NormalizedModalRow {
  return {
    label: item.label ?? item.text ?? "",
    detail: item.detail,
    hint: item.hint,
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
