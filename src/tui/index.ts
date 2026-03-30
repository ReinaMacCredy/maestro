/**
 * TUI entry point -- renderDashboard() event loop.
 * Composes panels into a full-screen frame.
 */
import { Screen } from "./terminal/screen.js";
import { startKeyListener, type Key } from "./terminal/input.js";
import { Buffer } from "./terminal/buffer.js";
import { splitV, splitH } from "./terminal/layout.js";
import type { MissionControlSnapshot } from "./types.js";
import type { SnapshotDeps } from "./snapshot.js";
import { buildSnapshot } from "./snapshot.js";
import { renderHeader } from "./panels/header.js";
import { renderStatusBar } from "./panels/status-bar.js";
import { renderFeatureDetail } from "./panels/feature-detail.js";
import { renderFeatureList } from "./panels/feature-list.js";
import { renderProgressLog } from "./panels/progress-log.js";
import { renderWorkerPanel } from "./panels/worker.js";
import { renderFooter } from "./panels/footer.js";

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
  renderFrame(buf, opts.snapshot);
  return buf.toString();
}

/**
 * Start the interactive dashboard event loop.
 * Resolves when the user exits (q or Ctrl+T).
 */
export async function renderDashboard(opts: InteractiveOptions): Promise<void> {
  const screen = new Screen();
  let running = true;
  let snapshot = opts.snapshot;
  let selectedFeatureIndex = 0;

  const handleKey = (key: Key): void => {
    if (key.type === "char" && key.char === "q") {
      running = false;
    }
    if (key.type === "ctrl" && key.char === "t") {
      running = false;
    }
    if (key.type === "ctrl" && key.char === "c") {
      running = false;
    }
    if (key.type === "arrow" && key.direction === "down") {
      if (selectedFeatureIndex < snapshot.features.length - 1) {
        selectedFeatureIndex++;
        render();
      }
    }
    if (key.type === "arrow" && key.direction === "up") {
      if (selectedFeatureIndex > 0) {
        selectedFeatureIndex--;
        render();
      }
    }
  };

  screen.enter();
  const stopKeys = startKeyListener(handleKey);

  const render = (): void => {
    const buf = screen.buffer();
    renderFrame(buf, snapshot, selectedFeatureIndex);
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
          snapshot = await buildSnapshot(opts.snapshotDeps, opts.missionId);
          render();
        } catch {
          // Poll failure is non-fatal
        }
      }, 2000);

      const check = setInterval(() => {
        if (!running) {
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

// ── Frame Composition ───────────────────────────────

function renderFrame(buf: Buffer, snap: MissionControlSnapshot, selectedIndex = 0): void {
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

  // Body: left detail (40%) + right feature list/log (60%)
  const [leftRect, rightRect] = splitH(bodyRect, [2, 3]);

  // Right pane: feature list (top 50%) + progress log (bottom 50%)
  const [featureListRect, progressRect] = splitV(rightRect!, [
    Math.max(3, Math.ceil(rightRect!.height * 0.5)),
    -1,
  ]);

  renderHeader(buf, headerRect, snap);
  renderStatusBar(buf, statusRect, snap);
  renderFeatureDetail(buf, leftRect!, snap);
  renderFeatureList(buf, featureListRect!, snap, selectedIndex);
  renderProgressLog(buf, progressRect!, snap.progressLog);
  renderWorkerPanel(buf, workerRect, snap);
  renderFooter(buf, footerRect, snap);
}
