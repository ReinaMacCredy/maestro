/**
 * TUI entry point -- renderDashboard() event loop.
 * Simple while-loop design: no setInterval, no double-buffer.
 */
import { Screen } from "./terminal/screen.js";
import { startKeyListener, type Key } from "./terminal/input.js";
import { Buffer } from "./terminal/buffer.js";
import { inset, type Rect } from "./terminal/layout.js";
import type { MissionControlSnapshot } from "./types.js";
import type { HomeSnapshotDeps, SnapshotDeps } from "./snapshot.js";
import { buildHomeSnapshot, buildSnapshot } from "./snapshot.js";
import { createInitialState, reduce, type AppState, type Action } from "./state.js";
import { HEADER_DOT_INTERVAL_MS, isHeaderAnimationActive, renderHeader } from "./panels/header.js";
import { renderStatusBar } from "./panels/status-bar.js";
import { renderFeatureDetail } from "./panels/feature-detail.js";
import { renderFeatureList } from "./panels/feature-list.js";
import { renderProgressLog } from "./panels/progress-log.js";
import { renderWorkerPanel } from "./panels/worker.js";
import { renderFooter } from "./panels/footer.js";
import {
  getFilteredMissionControlCommandSpecs,
  getMissionControlCommandSpecs,
  type MissionControlCommandId,
} from "./mission-control-commands.js";
import {
  applyModalBackdrop,
  layoutModal,
  pointInRect,
  renderModal,
  type ModalOptions,
} from "./widgets/modal.js";
import { getValidFeatureTransitions } from "../domain/mission-state.js";
import { FEATURE_STATUS_LABEL, PALETTE } from "./theme.js";
import { BOX } from "./terminal/ansi.js";
import { updateFeature } from "../usecases/feature-lifecycle.usecase.js";

export interface OnceFrameOptions {
  snapshot: MissionControlSnapshot;
}

export interface InteractiveOptions {
  snapshot: MissionControlSnapshot;
  snapshotDeps: SnapshotDeps;
  homeSnapshotDeps: HomeSnapshotDeps;
  missionId?: string;
}

/**
 * Render a single plain-text frame (for --once mode).
 */
export function renderOnceFrame(opts: OnceFrameOptions): string {
  const width = Math.min(process.stdout.columns || 120, 200);
  const minHeight = Math.max(opts.snapshot.features.length * 2 + 18, 22);
  const height = Math.max(process.stdout.rows || 0, minHeight);
  const buf = new Buffer(width, height);
  const state = createInitialState(opts.snapshot);
  renderFrame(buf, state, 0, 0);
  return buf.toString();
}

/**
 * Start the interactive dashboard.
 * Simple while-loop: sleep, check input, poll snapshot, render.
 */
export async function renderDashboard(opts: InteractiveOptions): Promise<void> {
  const screen = new Screen();
  let state = createInitialState(opts.snapshot);
  let shuttingDown = false;
  let animationFrame = 0;
  let nextAnimationTickMs = Date.now() + HEADER_DOT_INTERVAL_MS;
  let nextDurationTickMs = Date.now() + 1000;
  let lastSnapshotMs = Date.now();

  function sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  const requestQuit = (): void => {
    if (shuttingDown) return;
    shuttingDown = true;
    state = reduce(state, { type: "quit" });
  };

  // Key handler sets dirty flag
  let dirty = true;
  async function processKey(key: Key): Promise<void> {
    if (shuttingDown) return;
    if (key.type === "mouse") {
      await handleMouse(key);
      return;
    }

    const action = keyToAction(key, state);
    if (!action) return;

    if (action.type === "enter" && state.modal.kind === "command-palette") {
      const paletteAction = getCommandPaletteSelectionAction(state);
      if (!paletteAction) return;
      state = reduce(state, paletteAction);
      if (paletteAction.type === "quit") shuttingDown = true;
      dirty = true;
      return;
    }

    if (action.type === "enter" && shouldSubmitFeatureAction(state)) {
      await submitFeatureAction();
      return;
    }

    state = reduce(state, action);
    if (action.type === "quit") shuttingDown = true;
    dirty = true;
  }

  let inputQueue = Promise.resolve();
  const handleKey = (key: Key): void => {
    inputQueue = inputQueue
      .then(() => processKey(key))
      .catch(() => {
        requestQuit();
        dirty = true;
      });
  };

  // SIGINT/SIGTERM cleanup
  const handleSignal = (): void => {
    requestQuit();
    dirty = true;
  };
  process.on("SIGINT", handleSignal);
  process.on("SIGTERM", handleSignal);

  screen.enter();
  const stopKeys = startKeyListener(handleKey);

  try {
    // Initial render
    const buf = screen.createBuffer();
    renderFrame(buf, state, animationFrame, 0);
    screen.render(buf);
    dirty = false;

    let lastPollMs = Date.now();

    while (state.running) {
      await sleep(100);
      if (!state.running) break;

      // Check for terminal resize
      if (screen.refreshSize()) dirty = true;

      // Poll snapshot every 2s
      const now = Date.now();
      if (isHeaderAnimationActive(state.snapshot)) {
        if (now >= nextAnimationTickMs) {
          const elapsedTicks = Math.max(1, Math.floor((now - nextAnimationTickMs) / HEADER_DOT_INTERVAL_MS) + 1);
          animationFrame = (animationFrame + elapsedTicks) % 4;
          nextAnimationTickMs += elapsedTicks * HEADER_DOT_INTERVAL_MS;
          dirty = true;
        }
      } else if (animationFrame !== 0) {
        animationFrame = 0;
        nextAnimationTickMs = now + HEADER_DOT_INTERVAL_MS;
        dirty = true;
      } else {
        nextAnimationTickMs = now + HEADER_DOT_INTERVAL_MS;
      }

      if (now >= nextDurationTickMs) {
        const elapsedTicks = Math.max(1, Math.floor((now - nextDurationTickMs) / 1000) + 1);
        nextDurationTickMs += elapsedTicks * 1000;
        dirty = true;
      }

      if (now - lastPollMs >= 2000) {
        lastPollMs = now;
        try {
          const snap = opts.missionId
            ? await buildSnapshot(opts.snapshotDeps, opts.missionId)
            : await buildHomeSnapshot(opts.homeSnapshotDeps, process.cwd());
          state = reduce(state, { type: "update-snapshot", snapshot: snap });
          lastSnapshotMs = now;
          nextDurationTickMs = now + 1000;
          dirty = true;
        } catch {
          // non-fatal
        }
      }

      // Render only when something changed
      if (dirty) {
        const buf = screen.createBuffer();
        renderFrame(buf, state, animationFrame, now - lastSnapshotMs);
        screen.render(buf);
        dirty = false;
      }
    }
  } finally {
    stopKeys();
    screen.exit();
    process.off("SIGINT", handleSignal);
    process.off("SIGTERM", handleSignal);
  }

  async function handleMouse(key: Extract<Key, { type: "mouse" }>): Promise<void> {
    if (key.button !== "left" || key.event !== "down" || state.modal.kind === "none") return;

    const layout = getActiveModalLayout(screen.width, screen.height, state);
    if (!layout) return;

    if (!pointInRect(layout, key.x, key.y)) {
      state = reduce(state, { type: "escape" });
      dirty = true;
      return;
    }

    if (state.modal.kind === "command-palette") {
      const optionIndex = layout.itemRects.findIndex((rect) => pointInRect(rect, key.x, key.y));
      if (optionIndex < 0) return;

      const commands = getFilteredCommandPaletteItems(state);
      const command = commands[optionIndex];
      if (!command) return;

      state = reduce(state, command.action);
      if (command.action.type === "quit") shuttingDown = true;
      dirty = true;
      return;
    }

    if (state.modal.kind !== "feature-action" || state.modal.phase === "submitting") {
      if (state.modal.kind === "feature-browser") {
        const optionIndex = layout.itemRects.findIndex((rect) => pointInRect(rect, key.x, key.y));
        if (optionIndex < 0) return;
        state = reduce(state, { type: "modal-select", option: optionIndex });
        state = reduce(state, { type: "enter" });
        dirty = true;
      }
      return;
    }

    const optionIndex = layout.itemRects.findIndex((rect) => pointInRect(rect, key.x, key.y));
    if (optionIndex < 0) return;

    if (
      optionIndex === state.modal.selectedOption
      && (state.modal.phase === "confirming" || state.modal.phase === "error")
    ) {
      await submitFeatureAction();
      return;
    }

    state = reduce(state, { type: "modal-select", option: optionIndex });
    dirty = true;
  }

  async function submitFeatureAction(): Promise<void> {
    if (state.modal.kind !== "feature-action") return;

    const feature = state.snapshot.features[state.modal.featureIndex];
    if (!feature) return;

    const transitions = getValidFeatureTransitions(feature.status);
    const nextStatus = transitions[state.modal.selectedOption];
    if (!nextStatus) return;

    state = reduce(state, { type: "modal-submit-start" });
    dirty = true;

    try {
      await updateFeature(
        opts.snapshotDeps.missionStore,
        opts.snapshotDeps.featureStore,
        process.cwd(),
        state.snapshot.missionId,
        feature.id,
        { status: nextStatus },
      );

      try {
        const nextSnapshot = await buildSnapshot(opts.snapshotDeps, state.snapshot.missionId);
        state = reduce(state, { type: "update-snapshot", snapshot: nextSnapshot });
      } catch {
        // Fall back to the next poll refresh if the immediate snapshot reload fails.
      }

      state = { ...state, modal: { kind: "none" } };
      dirty = true;
    } catch (error) {
      state = reduce(state, {
        type: "modal-submit-error",
        message: error instanceof Error ? error.message : "Failed to update feature",
      });
      dirty = true;
    }
  }
}

// ── Key Mapping ─────────────────────────────────────

export function keyToAction(key: Key, state: AppState): Action | undefined {
  if (key.type === "char" && key.char === "q" && state.modal.kind === "none") {
    return { type: "quit" };
  }
  if (key.type === "ctrl" && (key.char === "t" || key.char === "c")) {
    return { type: "quit" };
  }
  if (key.type === "ctrl" && key.char === "p") {
    return { type: "open-command-palette" };
  }
  if (key.type === "escape") {
    return { type: "escape" };
  }
  if (
    key.type === "arrow"
    && key.direction === "left"
    && (
      (state.modal.kind === "feature-browser"
        || state.modal.kind === "overview"
        || state.modal.kind === "handoffs"
        || state.modal.kind === "config"
        || state.modal.kind === "processes")
      && state.modal.returnTarget === "command-palette"
    )
  ) {
    return { type: "navigate", direction: "left" };
  }
  if (key.type === "arrow" && (key.direction === "up" || key.direction === "down")) {
    return { type: "navigate", direction: key.direction };
  }
  if ((key.type === "backspace" || key.type === "delete") && state.modal.kind === "command-palette") {
    return { type: "modal-query-backspace" };
  }
  if (key.type === "enter") {
    return { type: "enter" };
  }
  if (key.type === "char" && state.modal.kind === "command-palette") {
    return { type: "modal-query-append", char: key.char };
  }
  if (key.type === "char" && state.modal.kind === "none") {
    const hotkey = key.char.toUpperCase();
    const command = getMissionControlCommandSpecs(state.snapshot.mode)
      .find((spec) => spec.key === hotkey);
    if (command) {
      return actionForMissionControlCommand(command.id);
    }
    switch (hotkey) {
      case "L":
      case "W":
        return { type: "focus", panel: "log" };
    }
  }
  return undefined;
}

function shouldSubmitFeatureAction(state: AppState): boolean {
  return state.modal.kind === "feature-action"
    && (state.modal.phase === "confirming" || state.modal.phase === "error");
}

// ── Frame Composition ───────────────────────────────

export function renderFrame(
  buf: Buffer,
  state: AppState,
  animationFrame = 0,
  elapsedOffsetMs = 0,
): void {
  const snap = state.snapshot;
  const w = buf.width;
  const h = buf.height;
  if (w < 4 || h < 8) return;

  const borderStyle = { fg: PALETTE.border };
  const outerRect = { x: 0, y: 0, width: w, height: h };
  const innerRect = inset(outerRect, 1);
  buf.drawBorder(outerRect, borderStyle);

  const headerRect: Rect = { x: innerRect.x, y: innerRect.y, width: innerRect.width, height: 1 };
  const headerDividerY = headerRect.y + headerRect.height;
  const statusRect: Rect = { x: innerRect.x, y: headerDividerY + 1, width: innerRect.width, height: 1 };
  const statusDividerY = statusRect.y + statusRect.height;
  const bottomDividerY = h - 5;
  const spacerY = h - 4;
  const footerDividerY = h - 3;
  const footerRect: Rect = { x: innerRect.x, y: h - 2, width: innerRect.width, height: 1 };

  drawFullWidthDivider(buf, headerDividerY, borderStyle);
  drawFullWidthDivider(buf, statusDividerY, borderStyle);
  drawFullWidthDivider(buf, bottomDividerY, borderStyle);
  drawFullWidthDivider(buf, footerDividerY, borderStyle);
  buf.fillRect({ x: 1, y: spacerY, width: w - 2, height: 1 }, " ");

  const bodyY = statusDividerY + 1;
  const bodyBottomY = bottomDividerY - 1;
  const bodyHeight = Math.max(0, bodyBottomY - bodyY + 1);
  const maxWorkerHeight = Math.max(6, bodyHeight - 5);
  const workerHeight = Math.min(Math.max(7, Math.floor(bodyHeight * 0.3)), maxWorkerHeight);
  const topBodyHeight = Math.max(4, bodyHeight - workerHeight - 1);
  const topBodyRect: Rect = { x: innerRect.x, y: bodyY, width: innerRect.width, height: topBodyHeight };
  const workerDividerY = topBodyRect.y + topBodyRect.height;
  const workerRect: Rect = {
    x: innerRect.x,
    y: workerDividerY + 1,
    width: innerRect.width,
    height: Math.max(0, bodyBottomY - workerDividerY),
  };

  drawFullWidthDivider(buf, workerDividerY, borderStyle);

  const splitOffset = clamp(Math.round(innerRect.width * (11 / 20)), 20, Math.max(20, innerRect.width - 18));
  const bodySplitX = clamp(innerRect.x + splitOffset, innerRect.x + 12, innerRect.x + innerRect.width - 13);
  drawVerticalDivider(buf, bodySplitX, statusDividerY, bottomDividerY, borderStyle);
  buf.set(statusDividerY, bodySplitX, BOX.teeDown, borderStyle);
  buf.set(workerDividerY, bodySplitX, BOX.cross, borderStyle);
  buf.set(bottomDividerY, bodySplitX, BOX.teeUp, borderStyle);

  const leftRect: Rect = {
    x: innerRect.x,
    y: topBodyRect.y,
    width: Math.max(0, bodySplitX - innerRect.x),
    height: topBodyRect.height,
  };
  const rightRect: Rect = {
    x: bodySplitX + 1,
    y: topBodyRect.y,
    width: Math.max(0, innerRect.x + innerRect.width - bodySplitX - 1),
    height: topBodyRect.height,
  };
  const minFeatureHeight = Math.min(topBodyRect.height - 3, Math.max(4, snap.features.length + 2));
  const featureHeight = Math.min(
    Math.max(minFeatureHeight, Math.ceil(topBodyRect.height * 0.45)),
    Math.max(4, topBodyRect.height - 3),
  );
  const rightSplitY = topBodyRect.y + featureHeight;
  drawHorizontalRange(buf, rightSplitY, bodySplitX, w - 1, borderStyle, BOX.cross, BOX.teeLeft);

  const featureListRect: Rect = {
    x: rightRect.x,
    y: rightRect.y,
    width: rightRect.width,
    height: Math.max(0, rightSplitY - rightRect.y),
  };
  const progressRect: Rect = {
    x: rightRect.x,
    y: rightSplitY + 1,
    width: rightRect.width,
    height: Math.max(0, topBodyRect.y + topBodyRect.height - rightSplitY - 1),
  };

  renderHeader(buf, headerRect, snap, animationFrame);
  renderStatusBar(buf, statusRect, snap);
  renderFeatureDetail(buf, leftRect, snap);
  renderFeatureList(buf, featureListRect, snap, state.selectedFeatureIndex);
      renderProgressLog(buf, progressRect, snap.progressLog, snap, state.logScrollOffset);
  renderWorkerPanel(buf, workerRect, snap, elapsedOffsetMs);
  renderFooter(buf, footerRect, snap);

    // Modal overlay
    const modalRect = getModalParentRect(w, h);
    if (state.modal.kind !== "none" && modalRect.width > 0 && modalRect.height > 0) {
      renderModalOverlay(buf, modalRect, state);
    }
  }

function drawFullWidthDivider(buf: Buffer, y: number, s: { fg: number }): void {
  drawHorizontalRange(buf, y, 0, buf.width - 1, s, BOX.teeRight, BOX.teeLeft);
}

function drawHorizontalRange(
  buf: Buffer,
  y: number,
  startX: number,
  endX: number,
  s: { fg: number },
  leftCap: string,
  rightCap: string,
): void {
  if (startX > endX || y < 0 || y >= buf.height) return;
  buf.set(y, startX, leftCap, s);
  for (let x = startX + 1; x < endX; x++) {
    buf.set(y, x, BOX.horizontal, s);
  }
  if (endX > startX) {
    buf.set(y, endX, rightCap, s);
  }
}

function drawVerticalDivider(
  buf: Buffer,
  x: number,
  startY: number,
  endY: number,
  s: { fg: number },
): void {
  for (let y = startY + 1; y < endY; y++) {
    buf.set(y, x, BOX.vertical, s);
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function renderModalOverlay(buf: Buffer, parentRect: Rect, state: AppState): void {
  const opts = buildModalOptions(state);
  if (!opts) return;
  applyModalBackdrop(buf);
  renderModal(buf, parentRect, opts);
}

function getActiveModalLayout(width: number, height: number, state: AppState) {
  const opts = buildModalOptions(state);
  if (!opts || state.modal.kind === "none") return undefined;
  const parentRect = getModalParentRect(width, height);
  if (parentRect.width <= 0 || parentRect.height <= 0) return undefined;
  return layoutModal(parentRect, opts);
}

function getModalParentRect(width: number, height: number): Rect {
  const innerRect = inset({ x: 0, y: 0, width, height }, 1);
  const headerDividerY = innerRect.y + 1;
  const statusRectY = headerDividerY + 1;
  const statusDividerY = statusRectY + 1;
  const bodyY = statusDividerY + 1;
  const footerDividerY = height - 3;
  return {
    x: innerRect.x,
    y: bodyY,
    width: innerRect.width,
    height: Math.max(0, footerDividerY - bodyY),
  };
}

function buildModalOptions(state: AppState): ModalOptions | undefined {
  if (state.modal.kind === "command-palette") {
    const commands = getFilteredCommandPaletteItems(state);
    return {
      mode: "palette",
      title: "Commands",
      query: state.modal.query,
      items: commands.map((command) => ({
        label: command.label,
        detail: command.detail,
        hint: command.hint,
        section: command.section,
      })),
      selectedIndex: Math.min(
        state.modal.selectedCommandIndex,
        Math.max(0, commands.length - 1),
      ),
      footer: "Enter open · Esc close",
      emptyLabel: "No commands match your filter",
    };
  }

  if (state.modal.kind === "feature-action") {
    const feature = state.snapshot.features[state.modal.featureIndex];
    if (!feature) return undefined;

    const transitions = getValidFeatureTransitions(feature.status);
    return {
      mode: "menu",
      title: "Change Feature Status",
      eyebrow: `${feature.id} · ${feature.title}`,
      items: transitions.length > 0
        ? transitions.map((transition) => ({
          label: `Set status to ${transition}`,
          detail: `Move ${feature.id} from ${FEATURE_STATUS_LABEL[feature.status]} to ${transition}`,
          section: "Transitions",
        }))
        : [{ label: "No valid transitions", detail: "This feature cannot move to another state right now.", section: "Transitions", tone: "muted" }],
      selectedIndex: state.modal.selectedOption,
      footer: getFeatureActionFooter(state.modal),
    };
  }

  if (state.modal.kind === "feature-browser") {
    return {
      mode: "menu",
      title: "Features",
      eyebrow: state.snapshot.mode === "home" ? "Project overview" : "Select a feature to focus",
      items: state.snapshot.features.length > 0
        ? state.snapshot.features.map((feature) => ({
          label: feature.title,
          detail: `${feature.id} · ${FEATURE_STATUS_LABEL[feature.status]} · ${feature.workerType}`,
          hint: feature.hasReport ? "report" : undefined,
          section: "Mission",
        }))
        : [{ label: "No features available", detail: "This mission does not have any features yet.", section: "Mission", tone: "muted" }],
      selectedIndex: Math.min(
        state.modal.selectedFeatureIndex,
        Math.max(0, state.snapshot.features.length - 1),
      ),
        footer: state.modal.returnTarget === "command-palette"
          ? "Enter focus · Left back · Esc close"
          : "Enter focus · Esc close",
    };
  }

  if (state.modal.kind === "overview" && state.snapshot.home) {
    return {
      mode: "info",
      title: "Overview",
      eyebrow: state.snapshot.home.headline,
      items: [
        { text: state.snapshot.home.summary, section: "Environment" },
        { text: state.snapshot.home.locationLabel, style: "block", tone: "accent", section: "Location" },
        ...state.snapshot.home.actions.map((action) => ({
          text: action.command,
          detail: action.detail,
          hint: action.label,
          section: "Next Steps",
          tone: "muted" as const,
        })),
      ],
      footer: state.modal.returnTarget === "command-palette" ? "Left back · Esc close" : "Esc close",
    };
  }

  if (state.modal.kind === "handoffs") {
    return {
      mode: "info",
      title: "Handoffs",
        eyebrow: state.snapshot.pendingHandoffs.length > 0
          ? `${state.snapshot.pendingHandoffs.length} pending`
          : "No pending handoffs",
        items: state.snapshot.pendingHandoffs.length > 0
          ? state.snapshot.pendingHandoffs.flatMap((handoff) => ([
            {
              text: `${handoff.id} · ${handoff.agent}`,
              detail: handoff.message,
              style: "block",
              tone: "accent" as const,
              section: "Pending",
            },
          ]))
          : [{ text: "No pending handoffs in this workspace.", section: "Pending", tone: "muted" }],
        footer: state.modal.returnTarget === "command-palette" ? "Left back · Esc close" : "Esc close",
      };
    }

  if (state.modal.kind === "config") {
    const summary = state.snapshot.configSummary;
    if (!summary) return undefined;
      return {
        mode: "info",
        title: "Config",
        eyebrow: `Config source: ${summary.configSource}`,
        items: [
          ...(summary.missionDirectory
            ? [{ text: summary.missionDirectory, style: "block" as const, tone: "accent" as const, section: "Mission Directory" }]
            : []),
          { text: `Git ${summary.gitAvailable ? "available" : "unavailable"}`, section: "Environment" },
          { text: `CASS ${summary.cassAvailable ? "available" : "unavailable"}`, section: "Environment" },
          ...summary.checks.map((check) => ({
            text: check.name,
            detail: `${check.status} · ${check.message}`,
            section: "Doctor",
            tone: "muted" as const,
          })),
          ...summary.workerTypes.map((workerType) => ({
            text: workerType,
            detail: "Worker model available for this mission",
            section: "Workers",
            tone: "muted" as const,
          })),
        ],
        footer: state.modal.returnTarget === "command-palette" ? "Left back · Esc close" : "Esc close",
      };
    }

  if (state.modal.kind === "processes") {
    return {
      mode: "info",
        title: "Processes",
        eyebrow: state.snapshot.runtimeProcesses.length > 0
          ? `${state.snapshot.runtimeProcesses.length} runtime item${state.snapshot.runtimeProcesses.length === 1 ? "" : "s"}`
          : "No active runtime processes",
        items: state.snapshot.runtimeProcesses.length > 0
          ? state.snapshot.runtimeProcesses.flatMap((process) => ([
            {
              text: process.title,
              detail: `${process.featureId} · ${FEATURE_STATUS_LABEL[process.status]} · ${process.workerType}${process.isLive ? " · live" : ""}${process.hasReport ? " · report available" : " · waiting for report"}`,
              style: "block" as const,
              tone: "accent" as const,
              section: "Runtime",
            },
          ]))
          : [{ text: "No assigned, in-progress, or review features right now.", section: "Runtime", tone: "muted" }],
        footer: state.modal.returnTarget === "command-palette" ? "Left back · Esc close" : "Esc close",
      };
    }

  return undefined;
}

function getFeatureActionFooter(modal: Extract<AppState["modal"], { kind: "feature-action" }>): string {
  if (modal.phase === "submitting") {
    return "Applying status...";
  }
  if (modal.phase === "error") {
    return `${modal.errorMessage ?? "Failed to update feature"} · Enter retry · Esc cancel`;
  }
  if (modal.phase === "confirming") {
    return "Enter confirm · Esc cancel";
  }
  return "Use arrows or click · Enter choose · Esc cancel";
}

interface CommandPaletteItem {
  readonly id: MissionControlCommandId;
  readonly label: string;
  readonly detail: string;
  readonly hint: string;
  readonly section: string;
  readonly keywords: readonly string[];
  readonly action: Action;
}

function getCommandPaletteItems(state: AppState): readonly CommandPaletteItem[] {
  return getMissionControlCommandSpecs(state.snapshot.mode).map((command) => ({
    id: command.id,
    label: command.label,
    detail: command.detail,
    hint: command.key,
    section: command.section,
    keywords: command.keywords,
    action: actionForMissionControlCommand(command.id),
  }));
}

function getFilteredCommandPaletteItems(state: AppState): readonly CommandPaletteItem[] {
  if (state.modal.kind !== "command-palette") return [];

  const filteredCommands = getFilteredMissionControlCommandSpecs(
    state.snapshot.mode,
    state.modal.query,
  );
  const itemsById = new Map(getCommandPaletteItems(state).map((item) => [item.id, item]));
  return filteredCommands
    .map((command) => itemsById.get(command.id))
    .filter((item): item is CommandPaletteItem => item !== undefined);
}

function getCommandPaletteSelectionAction(state: AppState): Action | undefined {
  if (state.modal.kind !== "command-palette") return undefined;

  const commands = getFilteredCommandPaletteItems(state);
  if (commands.length === 0) return undefined;

  const index = Math.min(state.modal.selectedCommandIndex, commands.length - 1);
  return commands[index]?.action;
}

function actionForMissionControlCommand(id: MissionControlCommandId): Action {
  switch (id) {
    case "features":
      return { type: "open-features" };
    case "handoffs":
      return { type: "open-handoffs" };
    case "config":
      return { type: "open-config" };
    case "processes":
      return { type: "open-processes" };
    case "exit":
      return { type: "quit" };
  }
}
