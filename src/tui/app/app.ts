/**
 * Interactive dashboard main loop.
 * Extracted from index.ts -- owns the event loop, signal handling, and snapshot polling.
 */
import { Screen } from "../terminal/screen.js";
import { startKeyListener, type Key } from "../terminal/input.js";
import type { MissionControlSnapshot } from "../state/types.js";
import type { SnapshotDeps } from "../state/snapshot.js";
import { createInitialState, reduce } from "../state/reducer.js";
import { getConfigRowsForTab } from "../state/config-inspector.js";
import { HEADER_DOT_INTERVAL_MS, isHeaderAnimationActive } from "../panels/header.js";
import { pointInRect } from "../widgets/modal.js";
import { getValidFeatureTransitions } from "../../domain/mission-state.js";
import { updateFeature } from "../../usecases/feature-lifecycle.usecase.js";
import { applyConfigEdit, previewConfigEdit } from "../../usecases/config-edit.usecase.js";
import { renderFrame, getActiveModalLayout } from "./render.js";
import {
  isSelectableListModal,
  getFilteredCommandPaletteItems,
  getCommandPaletteSelectionAction,
} from "./modal-builders.js";
import { keyToAction, shouldSubmitFeatureAction } from "./input-dispatch.js";

export interface InteractiveOptions {
  snapshot: MissionControlSnapshot;
  snapshotDeps: SnapshotDeps;
  reloadSnapshot: () => Promise<MissionControlSnapshot>;
}

export function getSnapshotPollIntervalMs(snapshot: MissionControlSnapshot): number {
  const hasActiveRuntime = snapshot.runtimeProcesses.some((process) =>
    process.isLive
    || process.runtimeState === "starting"
    || process.runtimeState === "stale"
    || process.runtimeState === "recoverable"
  );
  return hasActiveRuntime ? 1_000 : 2_000;
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

      if (action.type === "enter" && state.modal.kind === "config" && state.modal.phase === "edit-inline") {
        await prepareConfigReview();
        return;
      }

    if (action.type === "enter" && state.modal.kind === "config" && state.modal.phase === "confirm-write") {
        await submitConfigEdit();
        return;
      }

    if (action.type === "config-preview" && state.modal.kind === "config") {
      if (state.modal.phase === "browse") {
        const nextState = reduce(state, { type: "enter" });
        if (nextState !== state) {
          state = nextState;
          dirty = true;
        }
      }
      if (state.modal.phase === "edit-inline") {
        await prepareConfigReview();
      }
      return;
    }

    if (action.type === "config-reload" && state.modal.kind === "config") {
      try {
        const nextSnapshot = await opts.reloadSnapshot();
        state = reduce(state, { type: "update-snapshot", snapshot: nextSnapshot });
      } catch {
        // Non-fatal: keep the existing snapshot on screen.
      }
      dirty = true;
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
  screen.setMouseEnabled(!state.copyMode);
  const stopKeys = startKeyListener(handleKey);

  try {
    // Initial render
    const buf = screen.createBuffer();
    renderFrame(buf, state, animationFrame, 0);
    screen.setMouseEnabled(!state.copyMode);
    screen.render(buf);
    dirty = false;

    let lastPollMs = Date.now();

    while (state.running) {
      await sleep(100);
      if (!state.running) break;

      // Check for terminal resize
      if (screen.refreshSize()) dirty = true;

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

      if (now - lastPollMs >= getSnapshotPollIntervalMs(state.snapshot)) {
        lastPollMs = now;
          try {
            const snap = await opts.reloadSnapshot();
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
          screen.setMouseEnabled(!state.copyMode);
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
    if (state.copyMode || key.button !== "left" || key.event !== "down" || state.modal.kind === "none") return;

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
        if (isSelectableListModal(state.modal.kind)) {
          const optionIndex = layout.itemRects.findIndex((rect) => pointInRect(rect, key.x, key.y));
          if (optionIndex < 0) return;
          state = reduce(state, { type: "modal-select", option: optionIndex });
          if (state.modal.kind === "feature-browser") {
            state = reduce(state, { type: "enter" });
          }
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
          opts.snapshotDeps.runtimeStore,
          process.cwd(),
          state.snapshot.missionId,
          feature.id,
          { status: nextStatus },
        );

        try {
          const nextSnapshot = await opts.reloadSnapshot();
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

    async function submitConfigEdit(): Promise<void> {
      if (state.modal.kind !== "config" || state.modal.phase !== "confirm-write") return;

      const row = getConfigRowsForTab(
        state.snapshot.configInspector ?? null,
        state.modal.tab,
        state.modal.findQuery,
      )[state.modal.selectedRowIndex];
    if (!row) return;

    state = reduce(state, { type: "config-submit-start" });
    dirty = true;

    try {
      await previewConfigEdit(
        opts.snapshotDeps.config,
        process.cwd(),
        state.modal.selectedScope,
        row.keyPath,
        state.modal.draftValue ?? row.effectiveValueText,
      );
      await applyConfigEdit(
        opts.snapshotDeps.config,
        process.cwd(),
        state.modal.selectedScope,
        row.keyPath,
        state.modal.draftValue ?? row.effectiveValueText,
      );

      try {
        const nextSnapshot = await opts.reloadSnapshot();
        state = reduce(state, { type: "update-snapshot", snapshot: nextSnapshot });
      } catch {
        // Fall back to the next poll refresh if the immediate snapshot reload fails.
      }

      state = reduce(state, {
        type: "config-submit-success",
        message: `Updated ${row.keyPath} in ${state.modal.selectedScope} config`,
      });
      dirty = true;
    } catch (error) {
      state = reduce(state, {
        type: "config-submit-error",
        message: error instanceof Error ? error.message : "Failed to update config",
      });
        dirty = true;
      }
    }

    async function prepareConfigReview(): Promise<void> {
      if (state.modal.kind !== "config" || state.modal.phase !== "edit-inline") return;

      const row = getConfigRowsForTab(
        state.snapshot.configInspector ?? null,
        state.modal.tab,
        state.modal.findQuery,
      )[state.modal.selectedRowIndex];
      if (!row) return;

      try {
        const preview = await previewConfigEdit(
          opts.snapshotDeps.config,
          process.cwd(),
          state.modal.selectedScope,
          row.keyPath,
          state.modal.draftValue ?? row.effectiveValueText,
        );
        state = reduce(state, { type: "config-preview-ready", preview });
      } catch (error) {
        state = reduce(state, {
          type: "config-preview-error",
          message: error instanceof Error ? error.message : "Failed to build the config preview",
        });
      }

      dirty = true;
    }
  }
