/**
 * TUI entry point -- renderDashboard() event loop.
 * Composes panels into a full-screen frame with state-driven rendering.
 */
import { Screen } from "./terminal/screen.js";
import { startKeyListener, type Key } from "./terminal/input.js";
import { Buffer } from "./terminal/buffer.js";
import { splitV, splitH, type Rect } from "./terminal/layout.js";
import type { MissionControlSnapshot } from "./types.js";
import type { SnapshotDeps } from "./snapshot.js";
import { buildSnapshot } from "./snapshot.js";
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
import { truncate } from "./format.js";

export interface OnceFrameOptions {
  snapshot: MissionControlSnapshot;
}

export interface InteractiveOptions {
  snapshot: MissionControlSnapshot;
  snapshotDeps: SnapshotDeps;
  missionId: string;
}

/**
 * Render a single plain-text frame (for --once mode).
 */
export function renderOnceFrame(opts: OnceFrameOptions): string {
  const width = Math.min(process.stdout.columns || 120, 200);
  const height = Math.max(opts.snapshot.features.length * 2 + 12, 16);
  const buf = new Buffer(width, height);
  const state = createInitialState(opts.snapshot);
  renderFrame(buf, state);
  return buf.toString();
}

/**
 * Start the interactive dashboard event loop.
 * Resolves when the user exits (q or Ctrl+T).
 */
export async function renderDashboard(opts: InteractiveOptions): Promise<void> {
  const screen = new Screen();
  let state = createInitialState(opts.snapshot);

  const dispatch = (action: Action): void => {
    state = reduce(state, action);
    render();
  };

  const handleKey = (key: Key): void => {
    const action = keyToAction(key, state);
    if (action) dispatch(action);
  };

  screen.enter();
  const stopKeys = startKeyListener(handleKey);

  const render = (): void => {
    const buf = screen.buffer();
    renderFrame(buf, state);
    screen.flush();
  };

  screen.onResize(() => {
    screen.invalidate();
    render();
  });

  try {
    render();

    let pollTimer: ReturnType<typeof setInterval> | undefined;

    await new Promise<void>((resolve) => {
      pollTimer = setInterval(async () => {
        try {
          const snapshot = await buildSnapshot(opts.snapshotDeps, opts.missionId);
          dispatch({ type: "update-snapshot", snapshot });
        } catch {
          // Poll failure is non-fatal
        }
      }, 2000);

      const check = setInterval(() => {
        if (!state.running) {
          clearInterval(check);
          resolve();
        }
      }, 50);
    });

    if (pollTimer) clearInterval(pollTimer);
  } finally {
    stopKeys();
    screen.exit();
  }
}

// ── Key Mapping ─────────────────────────────────────

function keyToAction(key: Key, state: AppState): Action | undefined {
  // Exit shortcuts
  if (key.type === "char" && key.char === "q" && state.modal.kind === "none") {
    return { type: "quit" };
  }
  if (key.type === "ctrl" && (key.char === "t" || key.char === "c")) {
    return { type: "quit" };
  }

  // Modal escape
  if (key.type === "escape") {
    return { type: "escape" };
  }

  // Navigation
  if (key.type === "arrow" && (key.direction === "up" || key.direction === "down")) {
    return { type: "navigate", direction: key.direction };
  }

  // Enter
  if (key.type === "enter") {
    return { type: "enter" };
  }

  // Focus shortcuts (only when no modal)
  if (key.type === "char" && state.modal.kind === "none") {
    switch (key.char) {
      case "f":
      case "F":
        return { type: "focus", panel: "features" };
      case "w":
      case "W":
        return { type: "focus", panel: "log" };
      case "p":
      case "P":
        return { type: "toggle-pause" };
      case "d":
      case "D":
        return { type: "open-dir" };
      case "m":
      case "M":
        return { type: "open-models" };
    }
  }

  return undefined;
}

// ── Frame Composition ───────────────────────────────

function renderFrame(buf: Buffer, state: AppState): void {
  const snap = state.snapshot;
  const w = buf.width;
  const h = buf.height;

  // Layout zones
  const zones = splitV({ x: 0, y: 0, width: w, height: h }, [
    1,  // header
    1,  // status bar
    -1, // body (flex)
    3,  // worker panel
    1,  // footer
  ]);

  const headerRect = zones[0]!;
  const statusRect = zones[1]!;
  const bodyRect = zones[2]!;
  const workerRect = zones[3]!;
  const footerRect = zones[4]!;

  // Body: left detail (40%) + right pane (60%)
  const [leftRect, rightRect] = splitH(bodyRect, [2, 3]);

  // Right pane: feature list (top half) + progress log (bottom half)
  const [featureListRect, progressRect] = splitV(rightRect!, [
    Math.max(3, Math.ceil(rightRect!.height * 0.5)),
    -1,
  ]);

  renderHeader(buf, headerRect, snap);
  renderStatusBar(buf, statusRect, snap);
  renderFeatureDetail(buf, leftRect!, snap);
  renderFeatureList(buf, featureListRect!, snap, state.selectedFeatureIndex);
  renderProgressLog(buf, progressRect!, snap.progressLog);
  renderWorkerPanel(buf, workerRect, snap);
  renderFooter(buf, footerRect, snap);

  // Modal overlay (last, so it draws on top)
  if (state.modal.kind !== "none") {
    renderModalOverlay(buf, bodyRect, state);
  }
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
    const dir = `.maestro/missions/${state.snapshot.missionId}`;
    renderModal(buf, parentRect, {
      title: "Mission Directory",
      items: [dir],
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
