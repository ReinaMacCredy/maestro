/**
 * TUI entry point -- renderDashboard() event loop.
 * Simple while-loop design: no setInterval, no double-buffer.
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
 * Start the interactive dashboard.
 * Simple while-loop: sleep, check input, poll snapshot, render.
 */
export async function renderDashboard(opts: InteractiveOptions): Promise<void> {
  const screen = new Screen();
  let state = createInitialState(opts.snapshot);

  function sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  // Key handler sets dirty flag
  let dirty = true;
  const handleKey = (key: Key): void => {
    const action = keyToAction(key, state);
    if (action) {
      state = reduce(state, action);
      dirty = true;
    }
  };

  // SIGINT/SIGTERM cleanup
  const exitClean = (): void => { screen.exit(); process.exit(0); };
  process.on("SIGINT", exitClean);
  process.on("SIGTERM", exitClean);

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

      // Check for terminal resize
      if (screen.refreshSize()) dirty = true;

      // Poll snapshot every 2s
      const now = Date.now();
      if (now - lastPollMs >= 2000) {
        lastPollMs = now;
        try {
          const snap = await buildSnapshot(opts.snapshotDeps, opts.missionId);
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
    process.off("SIGINT", exitClean);
    process.off("SIGTERM", exitClean);
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
      case "w": case "W": return { type: "focus", panel: "log" };
      case "p": case "P": return { type: "toggle-pause" };
      case "d": case "D": return { type: "open-dir" };
      case "m": case "M": return { type: "open-models" };
    }
  }
  return undefined;
}

// ── Frame Composition ───────────────────────────────

function renderFrame(buf: Buffer, state: AppState): void {
  const snap = state.snapshot;
  const w = buf.width;
  const h = buf.height;

  const workerHeight = Math.max(5, Math.floor(h * 0.3));

  const zones = splitV({ x: 0, y: 0, width: w, height: h }, [
    1, 1, -1, workerHeight, 1,
  ]);

  const [headerRect, statusRect, bodyRect, workerRect, footerRect] = zones;

  const [leftRect, rightRect] = splitH(bodyRect!, [11, 9]);

  const [featureListRect, progressRect] = splitV(rightRect!, [
    Math.max(3, Math.ceil(rightRect!.height * 0.5)),
    -1,
  ]);

  renderHeader(buf, headerRect!, snap);
  renderStatusBar(buf, statusRect!, snap);
  renderFeatureDetail(buf, leftRect!, snap);
  renderFeatureList(buf, featureListRect!, snap, state.selectedFeatureIndex);
  renderProgressLog(buf, progressRect!, snap.progressLog);
  renderWorkerPanel(buf, workerRect!, snap);
  renderFooter(buf, footerRect!, snap);

  // Vertical separator
  if (rightRect) {
    for (let r = bodyRect!.y; r < bodyRect!.y + bodyRect!.height; r++) {
      buf.set(r, rightRect.x - 1, "\u2502", { fg: PALETTE.dimGray });
    }
  }

  // Modal overlay
  if (state.modal.kind !== "none") {
    renderModalOverlay(buf, bodyRect!, state);
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
