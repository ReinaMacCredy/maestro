/**
 * TUI application state -- focus, selection, modal management.
 */
import type { MissionControlSnapshot } from "./types.js";
import type { FeatureStatus } from "../domain/mission-types.js";

export type FocusedPanel = "features" | "log" | "none";

export type ModalState =
  | { kind: "none" }
  | { kind: "feature-action"; featureIndex: number; selectedOption: number; status?: string }
  | { kind: "directory" }
  | { kind: "models" };

export interface AppState {
  snapshot: MissionControlSnapshot;
  focusedPanel: FocusedPanel;
  selectedFeatureIndex: number;
  logScrollOffset: number;
  modal: ModalState;
  running: boolean;
}

export function createInitialState(snapshot: MissionControlSnapshot): AppState {
  return {
    snapshot,
    focusedPanel: "features",
    selectedFeatureIndex: 0,
    logScrollOffset: 0,
    modal: { kind: "none" },
    running: true,
  };
}

export type Action =
  | { type: "quit" }
  | { type: "navigate"; direction: "up" | "down" }
  | { type: "focus"; panel: FocusedPanel }
  | { type: "enter" }
  | { type: "escape" }
  | { type: "toggle-pause" }
  | { type: "open-dir" }
  | { type: "open-models" }
  | { type: "update-snapshot"; snapshot: MissionControlSnapshot }
  | { type: "modal-select"; option: number }
  | { type: "modal-status"; status: string };

/**
 * Pure state reducer -- returns a new state given an action.
 */
export function reduce(state: AppState, action: Action): AppState {
  switch (action.type) {
    case "quit":
      return { ...state, running: false };

    case "navigate": {
      if (state.modal.kind === "feature-action") {
        return handleModalNavigate(state, action.direction);
      }
      if (state.focusedPanel === "features") {
        return handleFeatureNavigate(state, action.direction);
      }
      if (state.focusedPanel === "log") {
        return handleLogNavigate(state, action.direction);
      }
      return state;
    }

    case "focus":
      if (state.modal.kind !== "none") return state;
      return { ...state, focusedPanel: action.panel };

    case "enter": {
      if (state.modal.kind === "feature-action") {
        return { ...state, modal: { ...state.modal, status: "confirm" } };
      }
      if (state.focusedPanel === "features" && state.snapshot.features.length > 0) {
        return {
          ...state,
          modal: {
            kind: "feature-action",
            featureIndex: state.selectedFeatureIndex,
            selectedOption: 0,
          },
        };
      }
      return state;
    }

    case "escape":
      if (state.modal.kind !== "none") {
        return { ...state, modal: { kind: "none" } };
      }
      return { ...state, focusedPanel: "none" };

    case "toggle-pause":
      return state; // Handled externally (needs store call)

    case "open-dir":
      if (state.modal.kind !== "none") return state;
      return { ...state, modal: { kind: "directory" } };

    case "open-models":
      if (state.modal.kind !== "none") return state;
      return { ...state, modal: { kind: "models" } };

    case "update-snapshot":
      return {
        ...state,
        snapshot: action.snapshot,
        selectedFeatureIndex: Math.min(
          state.selectedFeatureIndex,
          Math.max(0, action.snapshot.features.length - 1),
        ),
      };

    case "modal-select":
      if (state.modal.kind === "feature-action") {
        return { ...state, modal: { ...state.modal, selectedOption: action.option } };
      }
      return state;

    case "modal-status":
      if (state.modal.kind === "feature-action") {
        return { ...state, modal: { ...state.modal, status: action.status } };
      }
      return state;

    default:
      return state;
  }
}

function handleFeatureNavigate(state: AppState, direction: "up" | "down"): AppState {
  const total = state.snapshot.features.length;
  if (total === 0) return state;

  const newIndex = direction === "down"
    ? Math.min(state.selectedFeatureIndex + 1, total - 1)
    : Math.max(state.selectedFeatureIndex - 1, 0);

  if (newIndex === state.selectedFeatureIndex) return state;
  return { ...state, selectedFeatureIndex: newIndex };
}

function handleLogNavigate(state: AppState, direction: "up" | "down"): AppState {
  const total = state.snapshot.progressLog.length;
  if (total === 0) return state;

  const newOffset = direction === "down"
    ? Math.min(state.logScrollOffset + 1, Math.max(0, total - 5))
    : Math.max(state.logScrollOffset - 1, 0);

  return { ...state, logScrollOffset: newOffset };
}

function handleModalNavigate(state: AppState, direction: "up" | "down"): AppState {
  if (state.modal.kind !== "feature-action") return state;

  const feature = state.snapshot.features[state.modal.featureIndex];
  if (!feature) return state;

  // Options count comes from valid transitions for the feature
  const detail = state.snapshot.activeFeature;
  const optionsCount = detail?.validTransitions.length ?? 0;
  if (optionsCount === 0) return state;

  const newOption = direction === "down"
    ? Math.min(state.modal.selectedOption + 1, optionsCount - 1)
    : Math.max(state.modal.selectedOption - 1, 0);

  return { ...state, modal: { ...state.modal, selectedOption: newOption } };
}
