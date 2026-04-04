import { TextAttributes, type MouseEvent } from "@opentui/core";

import type { AppState } from "../../tui/state/reducer.js";
import type {
  InfoModalOptions,
  MenuModalOptions,
  ModalInfoItem,
  ModalOptions,
  ModalRow,
  PaletteModalOptions,
  SplitModalOptions,
  SplitModalRow,
} from "../../tui/widgets/modal.js";
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
        backgroundColor={OPEN_TUI_THEME.pageBg}
      >
        <box paddingLeft={1} paddingRight={1} paddingTop={1} flexDirection="column">
          <text fg={OPEN_TUI_THEME.accent} attributes={TextAttributes.BOLD}>Mission Control</text>
          <text fg={OPEN_TUI_THEME.text} attributes={TextAttributes.BOLD}>Terminal too small</text>
          <text fg={OPEN_TUI_THEME.muted}>
            Resize to at least 80x24 for the interactive dashboard, or use --size for deterministic previews.
          </text>
          <text fg={OPEN_TUI_THEME.muted}>{`Current: ${width}x${height}`}</text>
        </box>
      </box>
    );
  }

  const focusLines = buildFocusLines(state, contentWidth(layout.mainWidth), contentHeight(layout.leftTopHeight));
  const featureLines = buildFeatureListLines(state, contentWidth(layout.sideWidth), contentHeight(layout.rightTopHeight));
  const logLines = buildLogLines(state, contentWidth(layout.mainWidth), contentHeight(layout.leftBottomHeight));
  const sessionLines = buildSessionLines(state, contentWidth(layout.sideWidth), contentHeight(layout.rightBottomHeight), elapsedOffsetMs);
  const modal = buildModalModel(state);

  return (
    <box
      width={width}
      height={height}
      border
      flexDirection="column"
      backgroundColor={OPEN_TUI_THEME.pageBg}
      onMouseDown={onMouseDown}
    >
      <box width="100%" height={1} flexDirection="row" justifyContent="space-between">
        <text fg={header.left.fg} attributes={header.left.attributes}>{header.left.text}</text>
        <text fg={header.right.fg}>{header.right.text}</text>
      </box>

      <box width="100%" height={2} flexDirection="column">
        <box flexDirection="row" justifyContent="space-between">
          <text fg={status.primaryLeft.fg} attributes={status.primaryLeft.attributes}>{status.primaryLeft.text}</text>
          {status.primaryRight ? <text fg={status.primaryRight.fg} attributes={status.primaryRight.attributes}>{status.primaryRight.text}</text> : <box />}
        </box>
        <box flexDirection="row" justifyContent="space-between">
          {status.secondaryLeft ? <text fg={status.secondaryLeft.fg} attributes={status.secondaryLeft.attributes}>{status.secondaryLeft.text}</text> : <box />}
          {status.secondaryRight ? <text fg={status.secondaryRight.fg} attributes={status.secondaryRight.attributes}>{status.secondaryRight.text}</text> : <box />}
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
        />
      ) : (
        <SplitBody
          layout={layout}
          focusLines={focusLines}
          featureLines={featureLines}
          logLines={logLines}
          sessionLines={sessionLines}
          state={state}
        />
      )}

      <box width="100%" height={1} flexDirection="row" justifyContent="space-between" backgroundColor={OPEN_TUI_THEME.headerBg}>
        <text fg={state.copyMode ? OPEN_TUI_THEME.warning : OPEN_TUI_THEME.muted} attributes={state.copyMode ? TextAttributes.BOLD : undefined}>
          {footer.left}
        </text>
        <text fg={OPEN_TUI_THEME.muted}>{footer.right}</text>
      </box>

      {modal ? (
        <ModalLayer
          modal={modal}
          state={state}
          width={layout.modalWidth}
          height={layout.modalHeight}
          left={Math.max(1, Math.floor((layout.innerWidth - layout.modalWidth) / 2))}
          top={Math.max(1, Math.floor((layout.innerHeight - layout.modalHeight) / 2))}
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
}

function SplitBody({ layout, focusLines, featureLines, logLines, sessionLines, state }: BodyProps) {
  return (
    <box width="100%" height={layout.bodyHeight} flexDirection="row">
      <box width={layout.mainWidth} height={layout.bodyHeight} flexDirection="column" paddingRight={1}>
        <PanelFrame title={state.snapshot.mode === "home" ? "Overview" : state.leftPaneMode === "overview" ? "Mission Overview" : "Focus / Preview"} height={layout.leftTopHeight}>
          <LineList lines={focusLines} />
        </PanelFrame>
        <Spacer />
        <PanelFrame title={state.snapshot.mode === "home" ? "Pending Handoffs" : "Timeline"} height={layout.leftBottomHeight}>
          <LineList lines={logLines} />
        </PanelFrame>
      </box>

      <box width={layout.sideWidth} height={layout.bodyHeight} flexDirection="column">
        <PanelFrame title={state.snapshot.mode === "home" ? "Environment" : "Tasks"} height={layout.rightTopHeight}>
          <LineList lines={featureLines} />
        </PanelFrame>
        <Spacer />
        <PanelFrame title="Activity / Session" height={layout.rightBottomHeight}>
          <LineList lines={sessionLines} />
        </PanelFrame>
      </box>
    </box>
  );
}

function StackedBody({ layout, focusLines, featureLines, logLines, sessionLines, state }: BodyProps) {
  const [focusHeight, listHeight, logHeight, sessionHeight] = layout.stackedHeights;
  return (
    <box width="100%" height={layout.bodyHeight} flexDirection="column">
      <PanelFrame title={state.snapshot.mode === "home" ? "Overview" : state.leftPaneMode === "overview" ? "Mission Overview" : "Focus / Preview"} height={focusHeight}>
        <LineList lines={focusLines} />
      </PanelFrame>
      <Spacer />
      <PanelFrame title={state.snapshot.mode === "home" ? "Environment" : "Tasks"} height={listHeight}>
        <LineList lines={featureLines} />
      </PanelFrame>
      <Spacer />
      <PanelFrame title={state.snapshot.mode === "home" ? "Pending Handoffs" : "Timeline"} height={logHeight}>
        <LineList lines={logLines} />
      </PanelFrame>
      <Spacer />
      <PanelFrame title="Activity / Session" height={sessionHeight}>
        <LineList lines={sessionLines} />
      </PanelFrame>
    </box>
  );
}

interface PanelFrameProps {
  readonly title: string;
  readonly height: number;
  readonly children: React.ReactNode;
}

function PanelFrame({ title, height, children }: PanelFrameProps) {
  return (
    <box
      title={title}
      border
      width="100%"
      height={Math.max(3, height)}
      flexDirection="column"
      backgroundColor={OPEN_TUI_THEME.panelBg}
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
          <text fg={line.fg} attributes={line.attributes}>{line.text}</text>
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
}

function ModalLayer({ modal, state, width, height, left, top }: ModalLayerProps) {
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
      backgroundColor={OPEN_TUI_THEME.panelBgElevated}
      paddingLeft={1}
      paddingRight={1}
    >
      <box width="100%" flexDirection="row" justifyContent="space-between">
        <text fg={OPEN_TUI_THEME.accent} attributes={TextAttributes.BOLD}>{modal.title}</text>
        <text fg={OPEN_TUI_THEME.muted}>esc</text>
      </box>

      {eyebrowLines.map((line, index) => (
        <text key={index} fg={OPEN_TUI_THEME.muted}>{line}</text>
      ))}

      {modal.mode === "split" ? (
        <SplitModalBody modal={modal} width={width} height={height} />
      ) : modal.mode === "info" ? (
        <InfoModalBody modal={modal} />
      ) : modal.mode === "palette" ? (
        <PaletteModalBody modal={modal} />
      ) : (
        <MenuModalBody modal={modal} />
      )}

      {modal.footer ? (
        <box marginTop={1}>
          <text fg={OPEN_TUI_THEME.muted}>{modal.footer}</text>
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
        <text fg={OPEN_TUI_THEME.muted}>{modal.footer ?? "No items"}</text>
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
        <text fg={OPEN_TUI_THEME.warning}>{`/ ${modal.query.length > 0 ? modal.query : "type to filter"}`}</text>
      </box>
      {items.length === 0 ? (
        <text fg={OPEN_TUI_THEME.muted}>{modal.emptyLabel ?? "No commands match your filter"}</text>
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
}: {
  readonly modal: SplitModalOptions;
  readonly width: number;
  readonly height: number;
}) {
  const ratio = modal.renderSpec.layout.splitRatio ?? [46, 54];
  const total = ratio[0] + ratio[1];
  const leftWidth = Math.max(18, Math.floor((width - 3) * ratio[0] / total));
  const rightWidth = Math.max(18, width - 3 - leftWidth);
  const items = modal.items.map((item) => normalizeSplitModalRow(item));

  return (
    <box width="100%" flexGrow={1} flexDirection="row" marginTop={1}>
      <box width={leftWidth} height="100%" border title="List" paddingLeft={1} paddingRight={1} backgroundColor={OPEN_TUI_THEME.panelBg}>
        <box width="100%" height="100%" flexDirection="column">
          {items.length === 0 ? (
            <text fg={OPEN_TUI_THEME.muted}>{modal.emptyLabel ?? "No items"}</text>
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
      <box width={rightWidth} height="100%" border title="Detail" paddingLeft={1} paddingRight={1} backgroundColor={OPEN_TUI_THEME.panelBg}>
        <box width="100%" height="100%" flexDirection="column">
          {modal.detailItems.length === 0 ? (
            <text fg={OPEN_TUI_THEME.muted}>No details</text>
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
      <text fg={fg} attributes={selected ? TextAttributes.BOLD : row.style === "block" ? TextAttributes.BOLD : undefined}>
        {`${row.label}${detail}${hint}`}
      </text>
    </box>
  );
}

function InfoItemView({ item }: { readonly item: ModalInfoItem }) {
  const prefix = item.detail ? `${item.text}: ${item.detail}` : item.text;
  const attributes = item.style === "block" ? TextAttributes.BOLD : item.tone === "accent" ? TextAttributes.BOLD : undefined;
  return (
    <text fg={toneColor(item.tone)} attributes={attributes}>{prefix}</text>
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
