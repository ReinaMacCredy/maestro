import { createCliRenderer, MouseButton, type MouseEvent } from "@opentui/core";
import { createRoot, flushSync } from "@opentui/react";

import { getValidFeatureTransitions } from "../../../domain/mission-state.js";
import { applyConfigEdit, previewConfigEdit } from "../../../usecases/config-edit.usecase.js";
import { updateFeature } from "../../../usecases/feature-lifecycle.usecase.js";
import {
  getCommandPaletteSelectionAction,
  getFilteredCommandPaletteItems,
  isSelectableListModal,
} from "../../app/modal-builders.js";
import type { InteractiveOptions } from "../../app/interactive-shared.js";
import { keyToAction, shouldSubmitFeatureAction } from "../../app/input-dispatch.js";
import { getSnapshotPollIntervalMs } from "../../app/interactive-shared.js";
import { parseKeypress, type Key } from "../../input.js";
import { HEADER_DOT_INTERVAL_MS, isHeaderAnimationActive } from "../../shared/header-animation.js";
import { layoutModal, pointInRect } from "../../shared/modal-model.js";
import { getConfigRowsForTab, resolveConfigScopeForKey } from "../../state/config-inspector.js";
import { createInitialState, reduce } from "../../state/reducer.js";
import { MissionControlApp } from "./mission-control-app.js";
import { buildModalModel, computeScreenLayout, getModalParentRect } from "../components/builders.js";

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

const RESIZE_RENDER_INTERVAL_MS = 16;

export async function renderOpenTuiDashboard(opts: InteractiveOptions): Promise<void> {
  const renderer = await createCliRenderer({
    exitOnCtrlC: false,
    useMouse: true,
    screenMode: "alternate-screen",
  });
  const root = createRoot(renderer);

  let state = createInitialState(opts.snapshot);
  let shuttingDown = false;
  let dirty = true;
  let lastRenderedAnimationFrame = -1;
  let lastRenderMs = 0;
  let renderScheduled = false;
  let renderTimer: ReturnType<typeof setTimeout> | undefined;

  const getCurrentAnimationFrame = (): number => {
    if (!isHeaderAnimationActive(state.snapshot)) {
      return 0;
    }

    return Math.floor(Date.now() / HEADER_DOT_INTERVAL_MS);
  };

  const requestQuit = (): void => {
    if (shuttingDown) return;
    shuttingDown = true;
    state = reduce(state, { type: "quit" });
  };

  const renderCurrentFrame = (): void => {
    const animationFrame = getCurrentAnimationFrame();
    flushSync(() => {
      root.render(
        <MissionControlApp
          snapshot={state.snapshot}
          state={state}
          width={renderer.width}
          height={renderer.height}
          animationFrame={animationFrame}
          elapsedOffsetMs={0}
          onMouseDown={handleOpenTuiMouseDown}
        />,
      );
    });
    lastRenderedAnimationFrame = animationFrame;
    lastRenderMs = Date.now();
    renderer.useMouse = !state.copyMode;
  };

  const scheduleRender = (minimumDelayMs = 0): void => {
    if (shuttingDown || renderScheduled) return;

    const delayMs = Math.max(0, minimumDelayMs - (Date.now() - lastRenderMs));
    const runRender = (): void => {
      renderScheduled = false;
      renderTimer = undefined;
      if (!dirty || shuttingDown) return;
      renderCurrentFrame();
      dirty = false;
    };

    renderScheduled = true;
    if (delayMs === 0) {
      queueMicrotask(runRender);
      return;
    }

    renderTimer = setTimeout(runRender, delayMs);
  };

  const markDirty = (minimumDelayMs = 0): void => {
    dirty = true;
    scheduleRender(minimumDelayMs);
  };

  async function processKey(key: Key): Promise<void> {
    if (shuttingDown) return;
    if (key.type === "mouse") {
      await handleMouseDownAt(key.x, key.y);
      return;
    }

    const action = keyToAction(key, state);
    if (!action) return;

    if (action.type === "enter" && state.modal.kind === "command-palette") {
      const paletteAction = getCommandPaletteSelectionAction(state);
      if (!paletteAction) return;
      state = reduce(state, paletteAction);
      if (paletteAction.type === "quit") shuttingDown = true;
      markDirty();
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
          markDirty();
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
        // Keep the current snapshot when reload fails.
      }
      markDirty();
      return;
    }

    state = reduce(state, action);
    if (action.type === "quit") shuttingDown = true;
    markDirty();
  }

  let inputQueue = Promise.resolve();
  const queueTask = (task: () => Promise<void>): void => {
    inputQueue = inputQueue
      .then(task)
      .catch(() => {
        requestQuit();
        markDirty();
      });
  };

  const queueKey = (key: Key): void => {
    queueTask(() => processKey(key));
  };

  const handleOpenTuiMouseDown = (event: MouseEvent): void => {
    if (event.button !== MouseButton.LEFT) return;
    queueTask(() => handleMouseDownAt(event.x, event.y));
  };

  const handleRawInput = (sequence: string): boolean => {
    const keys = parseKeypress(new Uint8Array(Buffer.from(sequence, "utf8")));
    if (keys.length === 0) return false;
    for (const key of keys) {
      queueKey(key);
    }
    return true;
  };

  const handleResize = (): void => {
    markDirty(RESIZE_RENDER_INTERVAL_MS);
  };

  const handleSignal = (): void => {
    requestQuit();
    markDirty();
  };

  process.on("SIGINT", handleSignal);
  process.on("SIGTERM", handleSignal);
  renderer.prependInputHandler(handleRawInput);
  renderer.on("resize", handleResize);

  try {
    renderCurrentFrame();
    dirty = false;
    let lastPollMs = Date.now();

      while (state.running) {
        await sleep(100);
        if (!state.running) break;

        const now = Date.now();
        if (now - lastPollMs >= getSnapshotPollIntervalMs(state.snapshot)) {
          lastPollMs = now;
          try {
            const snapshot = await opts.reloadSnapshot();
            const nextState = reduce(state, { type: "update-snapshot", snapshot });
            state = nextState;
            markDirty();
          } catch {
            // Keep the current snapshot when polling fails.
          }
        }

        if (getCurrentAnimationFrame() !== lastRenderedAnimationFrame) {
          markDirty();
        }
      }
    } finally {
      if (renderTimer) {
        clearTimeout(renderTimer);
      }
      renderer.off("resize", handleResize);
      renderer.removeInputHandler(handleRawInput);
      process.off("SIGINT", handleSignal);
      process.off("SIGTERM", handleSignal);
      flushSync(() => {
        root.unmount();
      });
      renderer.destroy();
    }

  async function handleMouseDownAt(x: number, y: number): Promise<void> {
    if (state.copyMode || state.modal.kind === "none") return;

    const modal = buildModalModel(state);
    if (!modal) return;

    const screenLayout = computeScreenLayout(renderer.width, renderer.height, state.snapshot);
    const layout = layoutModal(getModalParentRect(screenLayout), modal);
    if (!layout) return;

    if (!pointInRect(layout, x, y)) {
      state = reduce(state, { type: "escape" });
      markDirty();
      return;
    }

    if (state.modal.kind === "command-palette") {
      const optionIndex = layout.itemRects.findIndex((rect) => pointInRect(rect, x, y));
      if (optionIndex < 0) return;

      const commands = getFilteredCommandPaletteItems(state);
      const command = commands[optionIndex];
      if (!command) return;

      state = reduce(state, command.action);
      if (command.action.type === "quit") shuttingDown = true;
      markDirty();
      return;
    }

    if (state.modal.kind !== "feature-action" || state.modal.phase === "submitting") {
      if (isSelectableListModal(state.modal.kind)) {
        const optionIndex = layout.itemRects.findIndex((rect) => pointInRect(rect, x, y));
        if (optionIndex < 0) return;
        state = reduce(state, { type: "modal-select", option: optionIndex });
        if (state.modal.kind === "feature-browser") {
          state = reduce(state, { type: "enter" });
        }
        markDirty();
      }
      return;
    }

    const optionIndex = layout.itemRects.findIndex((rect) => pointInRect(rect, x, y));
    if (optionIndex < 0) return;

    if (
      optionIndex === state.modal.selectedOption
      && (state.modal.phase === "confirming" || state.modal.phase === "error")
    ) {
      await submitFeatureAction();
      return;
    }

    state = reduce(state, { type: "modal-select", option: optionIndex });
    markDirty();
}

  async function submitFeatureAction(): Promise<void> {
    if (state.modal.kind !== "feature-action") return;

    const feature = state.snapshot.features[state.modal.featureIndex];
    if (!feature) return;

    const transitions = getValidFeatureTransitions(feature.status);
    const nextStatus = transitions[state.modal.selectedOption];
    if (!nextStatus) return;

    state = reduce(state, { type: "modal-submit-start" });
    markDirty();

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
      markDirty();
    } catch (error) {
      state = reduce(state, {
        type: "modal-submit-error",
        message: error instanceof Error ? error.message : "Failed to update feature",
      });
      markDirty();
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
    const selectedScope = resolveConfigScopeForKey(row.keyPath, state.modal.selectedScope);

    state = reduce(state, { type: "config-submit-start" });
    markDirty();

    try {
      await previewConfigEdit(
        opts.snapshotDeps.config,
        process.cwd(),
        selectedScope,
        row.keyPath,
        state.modal.draftValue ?? row.effectiveValueText,
      );
      await applyConfigEdit(
        opts.snapshotDeps.config,
        process.cwd(),
        selectedScope,
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
        message: `Updated ${row.keyPath} in ${selectedScope} config`,
      });
      markDirty();
    } catch (error) {
      state = reduce(state, {
        type: "config-submit-error",
        message: error instanceof Error ? error.message : "Failed to update config",
      });
      markDirty();
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
    const selectedScope = resolveConfigScopeForKey(row.keyPath, state.modal.selectedScope);

    try {
      const preview = await previewConfigEdit(
        opts.snapshotDeps.config,
        process.cwd(),
        selectedScope,
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

    markDirty();
  }
}
