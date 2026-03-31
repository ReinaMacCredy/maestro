/**
 * TUI application state -- focus, selection, modal management.
 */
import type { MissionControlSnapshot } from "./types.js";
import type { FeatureStatus } from "../domain/mission-types.js";
import { getValidFeatureTransitions } from "../domain/mission-state.js";

export type FocusedPanel = "features" | "log" | "none";

export type ModalState =
  | { kind: "none" }
  | {
    kind: "feature-action";
    featureIndex: number;
    selectedOption: number;
    phase: "selecting" | "confirming" | "submitting" | "error";
    errorMessage?: string;
  }
  | { kind: "feature-browser"; selectedFeatureIndex: number }
  | { kind: "overview" }
  | { kind: "handoffs" }
  | { kind: "config" }
  | { kind: "processes" };

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
  | { type: "open-features" }
  | { type: "open-handoffs" }
  | { type: "open-config" }
  | { type: "open-processes" }
  | { type: "update-snapshot"; snapshot: MissionControlSnapshot }
  | { type: "modal-select"; option: number }
  | { type: "modal-submit-start" }
  | { type: "modal-submit-error"; message: string };

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
      if (state.snapshot.mode === "home") {
        return state;
      }
      if (state.modal.kind === "feature-action") {
        if (state.modal.phase === "selecting") {
          return {
            ...state,
            modal: {
              ...state.modal,
              phase: "confirming",
              errorMessage: undefined,
            },
          };
        }
        return state;
      }
      if (state.modal.kind === "feature-browser") {
        return {
          ...state,
          focusedPanel: "features",
          selectedFeatureIndex: state.modal.selectedFeatureIndex,
          modal: { kind: "none" },
        };
      }
      if (state.focusedPanel === "features" && state.snapshot.features.length > 0) {
        return {
          ...state,
          modal: {
            kind: "feature-action",
            featureIndex: state.selectedFeatureIndex,
            selectedOption: 0,
            phase: "selecting",
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

    case "open-features":
      if (state.modal.kind !== "none") return state;
      if (state.snapshot.mode === "home") {
        return { ...state, modal: { kind: "overview" } };
      }
      return {
        ...state,
        modal: {
          kind: "feature-browser",
          selectedFeatureIndex: state.selectedFeatureIndex,
        },
      };

    case "open-handoffs":
      if (state.modal.kind !== "none") return state;
      return { ...state, modal: { kind: "handoffs" } };

    case "open-config":
      if (state.modal.kind !== "none") return state;
      return { ...state, modal: { kind: "config" } };

    case "open-processes":
      if (state.modal.kind !== "none") return state;
      return { ...state, modal: { kind: "processes" } };

    case "update-snapshot":
      const selectedFeatureId = state.snapshot.features[state.selectedFeatureIndex]?.id;
      const nextSelectedIndex = selectedFeatureId
        ? action.snapshot.features.findIndex((feature) => feature.id === selectedFeatureId)
        : -1;
      const baseState = {
        ...state,
        snapshot: action.snapshot,
        selectedFeatureIndex: nextSelectedIndex >= 0
          ? nextSelectedIndex
          : Math.min(
            state.selectedFeatureIndex,
            Math.max(0, action.snapshot.features.length - 1),
          ),
      };
      if (state.modal.kind === "feature-browser") {
        return {
          ...baseState,
          modal: {
            kind: "feature-browser",
            selectedFeatureIndex: Math.min(
              state.modal.selectedFeatureIndex,
              Math.max(0, action.snapshot.features.length - 1),
            ),
          },
        };
      }
      return {
        ...baseState,
      };

    case "modal-select":
      if (state.modal.kind === "feature-action") {
        return {
          ...state,
          modal: {
            ...state.modal,
            selectedOption: action.option,
            phase: "confirming",
            errorMessage: undefined,
          },
        };
      }
      if (state.modal.kind === "feature-browser") {
        return {
          ...state,
          modal: {
            kind: "feature-browser",
            selectedFeatureIndex: action.option,
          },
        };
      }
      return state;

    case "modal-submit-start":
      if (state.modal.kind === "feature-action") {
        return {
          ...state,
          modal: {
            ...state.modal,
            phase: "submitting",
            errorMessage: undefined,
          },
        };
      }
      return state;

    case "modal-submit-error":
      if (state.modal.kind === "feature-action") {
        return {
          ...state,
          modal: {
            ...state.modal,
            phase: "error",
            errorMessage: action.message,
          },
        };
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
  if (state.modal.kind === "feature-action") {
    const feature = state.snapshot.features[state.modal.featureIndex];
    if (!feature) return state;

    // Options count comes from valid transitions for the feature
    const optionsCount = getValidFeatureTransitions(feature.status).length;
    if (optionsCount === 0) return state;

    const newOption = direction === "down"
      ? Math.min(state.modal.selectedOption + 1, optionsCount - 1)
      : Math.max(state.modal.selectedOption - 1, 0);

    return {
      ...state,
      modal: {
        ...state.modal,
        selectedOption: newOption,
        phase: "selecting",
        errorMessage: undefined,
      },
    };
  }

  if (state.modal.kind === "feature-browser") {
    const total = state.snapshot.features.length;
    if (total === 0) return state;
    const selectedFeatureIndex = direction === "down"
      ? Math.min(state.modal.selectedFeatureIndex + 1, total - 1)
      : Math.max(state.modal.selectedFeatureIndex - 1, 0);
    return {
      ...state,
      modal: {
        kind: "feature-browser",
        selectedFeatureIndex,
      },
    };
  }

  return state;
}
