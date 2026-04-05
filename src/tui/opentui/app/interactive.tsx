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
import { HEADER_DOT_INTERVAL_MS, isHeaderAnimationActive } from "../../header-animation.js";
import { parseKeypress, type Key } from "../../input.js";
import { layoutModal, pointInRect } from "../../modal-model.js";
import { getConfigRowsForTab } from "../../state/config-inspector.js";
import { createInitialState, reduce, type AppState } from "../../state/reducer.js";
import { MissionControlApp } from "./mission-control-app.js";
import { buildModalModel, computeScreenLayout } from "../components/builders.js";

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

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
  let animationFrame = 0;
  let nextAnimationTickMs = Date.now() + HEADER_DOT_INTERVAL_MS;
  let nextDurationTickMs = Date.now() + 1000;
  let lastSnapshotMs = Date.now();

  const requestQuit = (): void => {
    if (shuttingDown) return;
    shuttingDown = true;
    state = reduce(state, { type: "quit" });
  };

  const renderCurrentFrame = (elapsedOffsetMs = 0): void => {
    flushSync(() => {
      root.render(
        <MissionControlApp
          snapshot={state.snapshot}
          state={state}
          width={renderer.width}
          height={renderer.height}
          animationFrame={animationFrame}
          elapsedOffsetMs={elapsedOffsetMs}
          onMouseDown={handleOpenTuiMouseDown}
        />,
      );
    });
    renderer.useMouse = !state.copyMode;
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
        // Keep the current snapshot when reload fails.
      }
      dirty = true;
      return;
    }

    state = reduce(state, action);
    if (action.type === "quit") shuttingDown = true;
    dirty = true;
  }

  let inputQueue = Promise.resolve();
  const queueTask = (task: () => Promise<void>): void => {
    inputQueue = inputQueue
      .then(task)
      .catch(() => {
        requestQuit();
        dirty = true;
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
    dirty = true;
  };

  const handleSignal = (): void => {
    requestQuit();
    dirty = true;
  };

  process.on("SIGINT", handleSignal);
  process.on("SIGTERM", handleSignal);
  renderer.prependInputHandler(handleRawInput);
  renderer.on("resize", handleResize);

  try {
    renderCurrentFrame(0);
    dirty = false;
    let lastPollMs = Date.now();

    while (state.running) {
      await sleep(100);
      if (!state.running) break;

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
          const snapshot = await opts.reloadSnapshot();
          state = reduce(state, { type: "update-snapshot", snapshot });
          lastSnapshotMs = now;
          nextDurationTickMs = now + 1000;
          dirty = true;
        } catch {
          // Keep the current snapshot when polling fails.
        }
      }

      if (dirty) {
        renderCurrentFrame(now - lastSnapshotMs);
        dirty = false;
      }
    }
  } finally {
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
    const layout = layoutModal({
      x: Math.max(1, Math.floor((screenLayout.innerWidth - screenLayout.modalWidth) / 2)),
      y: Math.max(1, Math.floor((screenLayout.innerHeight - screenLayout.modalHeight) / 2)),
      width: screenLayout.modalWidth,
      height: screenLayout.modalHeight,
    }, modal);
    if (!layout) return;

    if (!pointInRect(layout, x, y)) {
      state = reduce(state, { type: "escape" });
      dirty = true;
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
