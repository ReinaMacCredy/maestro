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
import { renderHeader } from "./panels/header.js";
import { renderStatusBar } from "./panels/status-bar.js";
import { renderFeatureDetail } from "./panels/feature-detail.js";
import { renderFeatureList } from "./panels/feature-list.js";
import { renderProgressLog } from "./panels/progress-log.js";
import { renderWorkerPanel } from "./panels/worker.js";
import { renderFooter } from "./panels/footer.js";
import { renderModal } from "./widgets/modal.js";
import { getValidFeatureTransitions } from "../domain/mission-state.js";
import { PALETTE } from "./theme.js";
import { BOX } from "./terminal/ansi.js";

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
  renderFrame(buf, state);
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
  const handleKey = (key: Key): void => {
    if (shuttingDown) return;
    const action = keyToAction(key, state);
    if (action) {
      state = reduce(state, action);
      if (action.type === "quit") shuttingDown = true;
      dirty = true;
    }
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
    renderFrame(buf, state);
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
        if (now - lastPollMs >= 2000) {
          lastPollMs = now;
          try {
            const snap = opts.missionId
              ? await buildSnapshot(opts.snapshotDeps, opts.missionId)
              : await buildHomeSnapshot(opts.homeSnapshotDeps, process.cwd());
            state = reduce(state, { type: "update-snapshot", snapshot: snap });
            dirty = true;
          } catch {
          // non-fatal
        }
      }

      // Render only when something changed
      if (dirty) {
        const buf = screen.createBuffer();
        renderFrame(buf, state);
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
}

// ── Key Mapping ─────────────────────────────────────

  function keyToAction(key: Key, state: AppState): Action | undefined {
  if (key.type === "char" && key.char === "q" && state.modal.kind === "none") {
    return { type: "quit" };
  }
  if (key.type === "ctrl" && (key.char === "t" || key.char === "c")) {
    return { type: "quit" };
  }
  if (key.type === "escape") {
    return { type: "escape" };
  }
  if (key.type === "arrow" && (key.direction === "up" || key.direction === "down")) {
    return { type: "navigate", direction: key.direction };
  }
  if (key.type === "enter") {
    return { type: "enter" };
  }
  if (key.type === "char" && state.modal.kind === "none") {
    switch (key.char) {
        case "f": case "F": return { type: "focus", panel: "features" };
        case "l": case "L":
        case "w": case "W":
          return { type: "focus", panel: "log" };
        case "p": case "P":
          return state.snapshot.mode === "mission" ? { type: "toggle-pause" } : undefined;
        case "d": case "D":
          return state.snapshot.mode === "mission" ? { type: "open-dir" } : undefined;
        case "m": case "M":
          return state.snapshot.mode === "mission" ? { type: "open-models" } : undefined;
      }
    }
  return undefined;
}

// ── Frame Composition ───────────────────────────────

function renderFrame(buf: Buffer, state: AppState): void {
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
  const workerHeight = Math.min(Math.max(2, Math.floor(bodyHeight * 0.2)), Math.max(2, bodyHeight - 6));
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

  renderHeader(buf, headerRect, snap);
  renderStatusBar(buf, statusRect, snap);
  renderFeatureDetail(buf, leftRect, snap);
  renderFeatureList(buf, featureListRect, snap, state.selectedFeatureIndex);
    renderProgressLog(buf, progressRect, snap.progressLog, snap);
  renderWorkerPanel(buf, workerRect, snap);
  renderFooter(buf, footerRect, snap);

  // Modal overlay
  const modalRect: Rect = {
    x: innerRect.x,
    y: bodyY,
    width: innerRect.width,
    height: Math.max(0, footerDividerY - bodyY),
  };
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
  if (state.modal.kind === "feature-action") {
    const feature = state.snapshot.features[state.modal.featureIndex];
    if (!feature) return;
    const transitions = getValidFeatureTransitions(feature.status);
    renderModal(buf, parentRect, {
      title: `${feature.id}: ${feature.title}`,
      items: transitions.length > 0
        ? transitions.map((t) => `Move to: ${t}`)
        : ["No valid transitions"],
      selectedIndex: state.modal.selectedOption,
      statusLine: state.modal.status,
    });
  }
  if (state.modal.kind === "directory") {
    renderModal(buf, parentRect, {
      title: "Mission Directory",
      items: [`.maestro/missions/${state.snapshot.missionId}`],
      selectedIndex: 0,
      statusLine: "Press Escape to close",
    });
  }
  if (state.modal.kind === "models") {
    const snap = state.snapshot;
    const workerTypes = [...new Set(snap.features.map((f) => f.workerType))];
    renderModal(buf, parentRect, {
      title: "Models & Workers",
      items: [
        `Mission: ${snap.missionId}`,
        `Status: ${snap.effectiveStatus}`,
        `Data source: store polling (2s)`,
        ...workerTypes.map((w) => `Worker: ${w}`),
      ],
      selectedIndex: 0,
      statusLine: "Press Escape to close",
    });
  }
}
