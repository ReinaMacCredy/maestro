/**
 * TUI application state -- focus, selection, modal management.
 */
import type {
  MissionControlConfigTab,
  MissionControlSnapshot,
  LeftPaneMode,
} from "./types.js";
import type { ConfigScope } from "../../ports/config.port.js";
import { getFilteredMissionControlPaletteCommandCount } from "./mission-control-commands.js";
import { getValidFeatureTransitions } from "../../domain/mission-state.js";
import { getConfigRowsForTab } from "./config-inspector.js";

export type FocusedPanel = "features" | "log" | "none";
export type { LeftPaneMode };

type ModalReturnTarget = "command-palette";

export type ModalState =
  | { kind: "none" }
  | { kind: "command-palette"; query: string; selectedCommandIndex: number }
  | {
    kind: "feature-action";
    featureIndex: number;
    selectedOption: number;
    phase: "selecting" | "confirming" | "submitting" | "error";
    errorMessage?: string;
  }
  | { kind: "feature-browser"; selectedFeatureIndex: number; returnTarget?: ModalReturnTarget }
  | { kind: "dependencies"; selectedOption: number; returnTarget?: ModalReturnTarget }
  | { kind: "overview"; returnTarget?: ModalReturnTarget }
  | { kind: "handoffs"; selectedHandoffIndex: number; returnTarget?: ModalReturnTarget }
    | {
      kind: "config";
      tab: MissionControlConfigTab;
      selectedRowIndex: number;
      phase: "browse" | "choose-scope" | "edit-inline" | "confirm-write" | "write-result";
      selectedScope: ConfigScope;
      findQuery?: string;
      draftValue?: string;
      message?: string;
      preview?: {
        scope: ConfigScope;
        path: string;
        content: string;
      };
      returnTarget?: ModalReturnTarget;
    }
  | { kind: "processes"; selectedProcessIndex: number; returnTarget?: ModalReturnTarget };

export interface AppState {
  snapshot: MissionControlSnapshot;
  focusedPanel: FocusedPanel;
  leftPaneMode: LeftPaneMode;
  copyMode: boolean;
  selectedFeatureIndex: number;
  logScrollOffset: number;
  modal: ModalState;
  running: boolean;
}

export function createInitialState(snapshot: MissionControlSnapshot): AppState {
  return {
    snapshot,
    focusedPanel: "features",
    leftPaneMode: "overview",
    copyMode: false,
    selectedFeatureIndex: 0,
    logScrollOffset: 0,
    modal: { kind: "none" },
    running: true,
  };
}

export type Action =
  | { type: "quit" }
  | { type: "navigate"; direction: "up" | "down" | "left" }
  | { type: "focus"; panel: FocusedPanel }
  | { type: "enter" }
  | { type: "escape" }
  | { type: "open-command-palette" }
  | { type: "open-features" }
  | { type: "open-dependencies" }
  | { type: "open-handoffs" }
  | { type: "open-config" }
  | { type: "open-processes" }
  | { type: "toggle-copy-mode" }
  | { type: "update-snapshot"; snapshot: MissionControlSnapshot }
  | { type: "modal-select"; option: number }
  | { type: "modal-query-append"; char: string }
  | { type: "modal-query-backspace" }
  | { type: "modal-submit-start" }
  | { type: "modal-submit-error"; message: string }
  | { type: "config-next-tab" }
  | { type: "config-prev-tab" }
    | { type: "config-cycle-value"; direction: "previous" | "next" }
    | { type: "config-toggle-scope" }
    | { type: "config-find-start" }
      | { type: "config-find-append"; char: string }
      | { type: "config-find-backspace" }
      | { type: "config-find-clear" }
      | { type: "config-preview-ready"; preview: { scope: ConfigScope; path: string; content: string } }
      | { type: "config-preview-error"; message: string }
      | { type: "config-submit-start" }
      | { type: "config-submit-success"; message: string }
      | { type: "config-submit-error"; message: string };

/**
 * Pure state reducer -- returns a new state given an action.
 */
export function reduce(state: AppState, action: Action): AppState {
  switch (action.type) {
    case "quit":
      return { ...state, running: false };

      case "navigate": {
        if (action.direction === "left") {
          return canNavigateBackToPalette(state.modal) ? closeOrReturnModal(state) : state;
        }

        if (
        state.modal.kind === "feature-action"
        || state.modal.kind === "command-palette"
        || state.modal.kind === "feature-browser"
        || state.modal.kind === "handoffs"
        || state.modal.kind === "processes"
        || state.modal.kind === "dependencies"
        || state.modal.kind === "config"
      ) {
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
      return {
        ...state,
        focusedPanel: action.panel,
        leftPaneMode: action.panel === "features" ? state.leftPaneMode : "overview",
      };

    case "enter": {
      if (state.modal.kind === "command-palette") {
        return state;
      }
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
          leftPaneMode: "preview",
          selectedFeatureIndex: state.modal.selectedFeatureIndex,
          modal: { kind: "none" },
        };
      }
        if (state.modal.kind === "handoffs") {
          return state;
        }
        if (state.modal.kind === "processes") {
          return state;
        }
      if (state.modal.kind === "dependencies") {
        const targetId = getSelectedDependencyTargetId(state);
        if (!targetId) return state;

        const nextIndex = state.snapshot.features.findIndex((feature) => feature.id === targetId);
        if (nextIndex < 0) {
          return { ...state, modal: { kind: "none" }, leftPaneMode: "preview" };
        }

        return {
          ...state,
          focusedPanel: "features",
          leftPaneMode: "preview",
          selectedFeatureIndex: nextIndex,
          modal: { kind: "none" },
        };
      }
      if (state.modal.kind === "config") {
        return handleConfigEnter(state);
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
          if (state.modal.kind === "config") {
            if (state.modal.phase === "browse" && state.modal.findQuery !== undefined) {
              return {
                ...state,
                modal: {
                  ...state.modal,
                  findQuery: undefined,
                  selectedRowIndex: 0,
                },
              };
            }
            if (state.modal.phase === "confirm-write") {
              return {
                ...state,
                modal: {
                  ...state.modal,
                  phase: "edit-inline",
                  preview: undefined,
                },
              };
            }
            if (
              state.modal.phase === "choose-scope"
            || state.modal.phase === "edit-inline"
            || state.modal.phase === "write-result"
          ) {
            return {
                ...state,
                modal: {
                  ...state.modal,
                  phase: state.modal.phase === "choose-scope" && state.modal.draftValue ? "edit-inline" : "browse",
                  findQuery: undefined,
                  draftValue: state.modal.phase === "choose-scope" ? state.modal.draftValue : undefined,
                  message: undefined,
                  preview: undefined,
                },
                copyMode: false,
              };
            }
        }
        return { ...state, modal: { kind: "none" }, copyMode: false };
      }
      if (state.copyMode) {
        return { ...state, copyMode: false };
      }
      if (state.leftPaneMode === "preview") {
        return { ...state, focusedPanel: "none", leftPaneMode: "overview" };
      }
      return { ...state, focusedPanel: "none" };

    case "open-command-palette":
      if (!canOpenOverlayFromModal(state.modal)) return state;
      return {
        ...state,
        modal: {
          kind: "command-palette",
          query: "",
          selectedCommandIndex: 0,
        },
      };

    case "open-features":
      if (!canOpenOverlayFromModal(state.modal)) return state;
      if (state.snapshot.mode === "home") {
        return {
          ...state,
          modal: { kind: "overview", returnTarget: getModalReturnTarget(state.modal) },
        };
      }
      return {
        ...state,
        modal: {
          kind: "feature-browser",
          selectedFeatureIndex: state.selectedFeatureIndex,
          returnTarget: getModalReturnTarget(state.modal),
        },
      };

    case "open-dependencies":
      if (!canOpenOverlayFromModal(state.modal)) return state;
      if (state.snapshot.mode !== "mission") return state;
      return {
        ...state,
        modal: {
          kind: "dependencies",
          selectedOption: 0,
          returnTarget: getModalReturnTarget(state.modal),
        },
      };

    case "open-handoffs":
      if (!canOpenOverlayFromModal(state.modal)) return state;
      return {
        ...state,
        modal: {
          kind: "handoffs",
          selectedHandoffIndex: 0,
          returnTarget: getModalReturnTarget(state.modal),
        },
      };

    case "open-config":
      if (!canOpenOverlayFromModal(state.modal)) return state;
        return {
          ...state,
            modal: {
              kind: "config",
              tab: "overview",
              selectedRowIndex: 0,
              phase: "browse",
              selectedScope: "project",
              findQuery: undefined,
              preview: undefined,
              returnTarget: getModalReturnTarget(state.modal),
            },
          };

    case "open-processes":
      if (!canOpenOverlayFromModal(state.modal)) return state;
      if (state.snapshot.mode !== "mission") return state;
      return {
        ...state,
        modal: {
          kind: "processes",
          selectedProcessIndex: 0,
          returnTarget: getModalReturnTarget(state.modal),
        },
      };

    case "toggle-copy-mode":
      return { ...state, copyMode: !state.copyMode };

    case "update-snapshot": {
      const selectedFeatureId = state.snapshot.features[state.selectedFeatureIndex]?.id;
      const nextSelectedIndex = selectedFeatureId
        ? action.snapshot.features.findIndex((feature) => feature.id === selectedFeatureId)
        : -1;
      const baseState: AppState = {
        ...state,
        snapshot: action.snapshot,
        selectedFeatureIndex: nextSelectedIndex >= 0
          ? nextSelectedIndex
          : Math.min(state.selectedFeatureIndex, Math.max(0, action.snapshot.features.length - 1)),
      };

      if (state.modal.kind === "feature-browser") {
        const selectedModalFeatureId = state.snapshot.features[state.modal.selectedFeatureIndex]?.id;
        const nextModalSelectedIndex = selectedModalFeatureId
          ? action.snapshot.features.findIndex((feature) => feature.id === selectedModalFeatureId)
          : -1;
        return {
          ...baseState,
          modal: {
            kind: "feature-browser",
            selectedFeatureIndex: nextModalSelectedIndex >= 0
              ? nextModalSelectedIndex
              : Math.min(state.modal.selectedFeatureIndex, Math.max(0, action.snapshot.features.length - 1)),
            returnTarget: state.modal.returnTarget,
          },
        };
      }

      if (state.modal.kind === "handoffs") {
        return {
          ...baseState,
          modal: {
            kind: "handoffs",
            selectedHandoffIndex: Math.min(
              state.modal.selectedHandoffIndex,
              Math.max(0, action.snapshot.pendingHandoffs.length - 1),
            ),
            returnTarget: state.modal.returnTarget,
          },
        };
      }

        if (state.modal.kind === "processes") {
          return {
            ...baseState,
          modal: {
            kind: "processes",
            selectedProcessIndex: Math.min(
              state.modal.selectedProcessIndex,
              Math.max(0, action.snapshot.runtimeProcesses.length - 1),
            ),
            returnTarget: state.modal.returnTarget,
          },
        };
      }

        if (state.modal.kind === "dependencies") {
          return {
            ...baseState,
          modal: {
            kind: "dependencies",
            selectedOption: Math.min(
              state.modal.selectedOption,
              Math.max(0, getDependencyTargets(baseState).length - 1),
            ),
            returnTarget: state.modal.returnTarget,
            },
          };
        }

          if (state.modal.kind === "config") {
            return {
              ...baseState,
              modal: {
                ...state.modal,
                selectedRowIndex: Math.min(
                  state.modal.selectedRowIndex,
                  Math.max(0, getConfigRowsForTab(action.snapshot.configInspector ?? null, state.modal.tab, state.modal.findQuery).length - 1),
                ),
              },
            };
          }

        return baseState;
      }

    case "modal-select":
      if (state.modal.kind === "command-palette") {
        return {
          ...state,
          modal: {
            ...state.modal,
            selectedCommandIndex: Math.max(0, action.option),
          },
        };
      }
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
            returnTarget: state.modal.returnTarget,
          },
        };
      }
      if (state.modal.kind === "handoffs") {
        return {
          ...state,
          modal: {
            kind: "handoffs",
            selectedHandoffIndex: action.option,
            returnTarget: state.modal.returnTarget,
          },
        };
      }
      if (state.modal.kind === "processes") {
        return {
          ...state,
          modal: {
            kind: "processes",
            selectedProcessIndex: action.option,
            returnTarget: state.modal.returnTarget,
          },
        };
      }
        if (state.modal.kind === "dependencies") {
          return {
            ...state,
          modal: {
            kind: "dependencies",
            selectedOption: action.option,
            returnTarget: state.modal.returnTarget,
            },
          };
        }
        if (state.modal.kind === "config") {
          return {
            ...state,
            modal: {
              ...state.modal,
              selectedRowIndex: action.option,
            },
          };
        }
        return state;

    case "modal-query-append":
      if (state.modal.kind === "command-palette") {
        return {
          ...state,
          modal: {
            ...state.modal,
            query: state.modal.query + action.char,
            selectedCommandIndex: 0,
          },
        };
      }
      return state;

      case "modal-query-backspace":
        if (state.modal.kind === "command-palette") {
          return {
            ...state,
          modal: {
            ...state.modal,
            query: state.modal.query.slice(0, -1),
            selectedCommandIndex: 0,
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

        case "config-find-start":
          if (state.modal.kind !== "config" || state.modal.phase !== "browse") return state;
          return {
            ...state,
            modal: {
              ...state.modal,
              findQuery: "",
              selectedRowIndex: 0,
            },
          };

        case "config-find-append":
          if (state.modal.kind !== "config" || state.modal.phase !== "browse") return state;
          return {
            ...state,
            modal: {
              ...state.modal,
              findQuery: `${state.modal.findQuery ?? ""}${action.char}`,
              selectedRowIndex: 0,
            },
          };

        case "config-find-backspace":
          if (state.modal.kind !== "config" || state.modal.phase !== "browse") return state;
          return {
            ...state,
            modal: {
              ...state.modal,
              findQuery: (state.modal.findQuery ?? "").slice(0, -1),
              selectedRowIndex: 0,
            },
          };

        case "config-find-clear":
          if (state.modal.kind !== "config") return state;
          return {
            ...state,
            modal: {
              ...state.modal,
              findQuery: undefined,
              selectedRowIndex: 0,
            },
          };

      case "config-next-tab":
      case "config-prev-tab":
        if (state.modal.kind !== "config") return state;
        return {
          ...state,
              modal: {
                ...state.modal,
                tab: nextConfigTab(state.modal.tab, action.type === "config-next-tab" ? 1 : -1),
                selectedRowIndex: 0,
                phase: "browse",
                findQuery: undefined,
                draftValue: undefined,
                message: undefined,
                preview: undefined,
              },
          };

      case "config-cycle-value":
        if (state.modal.kind !== "config" || state.modal.phase !== "edit-inline") return state;
        return cycleConfigDraft(state, action.direction);

        case "config-toggle-scope":
          if (state.modal.kind !== "config") return state;
          if (state.modal.phase === "choose-scope") {
            return {
              ...state,
              modal: {
                ...state.modal,
                selectedScope: state.modal.selectedScope === "project" ? "global" : "project",
                preview: undefined,
              },
            };
          }
          return {
            ...state,
            modal: {
              ...state.modal,
              phase: "choose-scope",
              findQuery: undefined,
              message: undefined,
              preview: undefined,
            },
          };

        case "config-preview-ready":
          if (state.modal.kind !== "config") return state;
          return {
            ...state,
            modal: {
              ...state.modal,
              phase: "confirm-write",
              message: undefined,
              preview: action.preview,
            },
          };

        case "config-preview-error":
          if (state.modal.kind !== "config") return state;
          return {
            ...state,
            modal: {
              ...state.modal,
              phase: "write-result",
              findQuery: undefined,
              message: action.message,
              preview: undefined,
            },
          };

        case "config-submit-start":
          if (state.modal.kind !== "config") return state;
          return {
            ...state,
            modal: {
            ...state.modal,
            message: undefined,
          },
        };

      case "config-submit-success":
      case "config-submit-error":
        if (state.modal.kind !== "config") return state;
        return {
          ...state,
              modal: {
                ...state.modal,
                phase: "write-result",
                findQuery: undefined,
                message: action.message,
                draftValue: undefined,
                preview: undefined,
              },
          };

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
  return { ...state, selectedFeatureIndex: newIndex, leftPaneMode: "preview" };
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
  if (state.modal.kind === "command-palette") {
    const optionsCount = getFilteredMissionControlPaletteCommandCount(
      state.snapshot.mode,
      state.modal.query,
    );
    if (optionsCount === 0) return state;

    const selectedCommandIndex = direction === "down"
      ? Math.min(state.modal.selectedCommandIndex + 1, optionsCount - 1)
      : Math.max(state.modal.selectedCommandIndex - 1, 0);
    return {
      ...state,
      modal: {
        ...state.modal,
        selectedCommandIndex,
      },
    };
  }

  if (state.modal.kind === "feature-action") {
    const feature = state.snapshot.features[state.modal.featureIndex];
    if (!feature) return state;

    const optionsCount = getValidFeatureTransitions(feature.status).length;
    if (optionsCount === 0) return state;

    const selectedOption = direction === "down"
      ? Math.min(state.modal.selectedOption + 1, optionsCount - 1)
      : Math.max(state.modal.selectedOption - 1, 0);

    return {
      ...state,
      modal: {
        ...state.modal,
        selectedOption,
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
        returnTarget: state.modal.returnTarget,
      },
    };
  }

  if (state.modal.kind === "handoffs") {
    const total = state.snapshot.pendingHandoffs.length;
    if (total === 0) return state;

    const selectedHandoffIndex = direction === "down"
      ? Math.min(state.modal.selectedHandoffIndex + 1, total - 1)
      : Math.max(state.modal.selectedHandoffIndex - 1, 0);

    return {
      ...state,
      modal: {
        kind: "handoffs",
        selectedHandoffIndex,
        returnTarget: state.modal.returnTarget,
      },
    };
  }

  if (state.modal.kind === "processes") {
    const total = state.snapshot.runtimeProcesses.length;
    if (total === 0) return state;

    const selectedProcessIndex = direction === "down"
      ? Math.min(state.modal.selectedProcessIndex + 1, total - 1)
      : Math.max(state.modal.selectedProcessIndex - 1, 0);

    return {
      ...state,
      modal: {
        kind: "processes",
        selectedProcessIndex,
        returnTarget: state.modal.returnTarget,
      },
    };
  }

  if (state.modal.kind === "dependencies") {
    const total = getDependencyTargets(state).length;
    if (total === 0) return state;

    const selectedOption = direction === "down"
      ? Math.min(state.modal.selectedOption + 1, total - 1)
      : Math.max(state.modal.selectedOption - 1, 0);

    return {
      ...state,
      modal: {
        kind: "dependencies",
        selectedOption,
        returnTarget: state.modal.returnTarget,
        },
      };
    }

    if (state.modal.kind === "config") {
      const total = getConfigRowsForTab(
        state.snapshot.configInspector ?? null,
        state.modal.tab,
        state.modal.findQuery,
      ).length;
      if (state.modal.phase === "choose-scope") {
        return {
          ...state,
          modal: {
            ...state.modal,
            selectedScope: direction === "down" ? "global" : "project",
            preview: undefined,
          },
        };
      }
      if (state.modal.phase === "edit-inline") {
        return cycleConfigDraft(state, direction === "down" ? "next" : "previous");
      }
      if (state.modal.phase !== "browse" || total === 0) return state;

    const selectedRowIndex = direction === "down"
      ? Math.min(state.modal.selectedRowIndex + 1, total - 1)
      : Math.max(state.modal.selectedRowIndex - 1, 0);

    return {
      ...state,
      modal: {
        ...state.modal,
        selectedRowIndex,
      },
    };
  }

  return state;
}

function canOpenOverlayFromModal(modal: ModalState): boolean {
  return modal.kind === "none" || modal.kind === "command-palette";
}

function closeOrReturnModal(state: AppState): AppState {
  if (state.modal.kind === "none") {
    return { ...state, focusedPanel: "none" };
  }
  if (state.modal.kind === "command-palette") {
    return { ...state, modal: { kind: "none" } };
  }

  const returnTarget = getModalReturnTarget(state.modal);
  if (returnTarget === "command-palette") {
    return {
      ...state,
      modal: {
        kind: "command-palette",
        query: "",
        selectedCommandIndex: 0,
      },
    };
  }

  return { ...state, modal: { kind: "none" } };
}

function getModalReturnTarget(modal: ModalState): ModalReturnTarget | undefined {
    if (
      modal.kind === "feature-browser"
      || modal.kind === "dependencies"
      || modal.kind === "overview"
      || modal.kind === "handoffs"
      || modal.kind === "config"
      || modal.kind === "processes"
    ) {
      return modal.returnTarget;
    }
  if (modal.kind === "command-palette") {
    return "command-palette";
  }
  return undefined;
}

function canNavigateBackToPalette(modal: ModalState): boolean {
  return modal.kind !== "command-palette" && getModalReturnTarget(modal) === "command-palette";
}

function getSelectedDependencyTargetId(state: AppState): string | undefined {
  if (state.modal.kind !== "dependencies") return undefined;
  return getDependencyTargets(state)[state.modal.selectedOption]?.id;
}

function getDependencyTargets(state: AppState): ReadonlyArray<{ id: string }> {
  const preview = state.snapshot.taskPreviews?.[state.selectedFeatureIndex] ?? state.snapshot.activeFeature;
  if (!preview) return [];

  return [
    ...(preview.blockedBy ?? []),
    ...(preview.unblocks ?? []),
  ].map((feature) => ({ id: feature.id }));
}

function handleConfigEnter(state: AppState): AppState {
  if (state.modal.kind !== "config") return state;

    const row = getConfigRowsForTab(
      state.snapshot.configInspector ?? null,
      state.modal.tab,
      state.modal.findQuery,
    )[state.modal.selectedRowIndex];
    if (!row) return state;

  if (state.modal.phase === "browse") {
    if (row.editKind === "readonly") {
      return state;
    }

    return {
      ...state,
      modal: {
        ...state.modal,
        phase: "edit-inline",
        findQuery: undefined,
        draftValue: row.effectiveValueText,
        message: undefined,
        preview: undefined,
      },
    };
  }

  if (state.modal.phase === "choose-scope") {
    return {
      ...state,
      modal: {
        ...state.modal,
        phase: state.modal.draftValue ? "edit-inline" : "browse",
        findQuery: undefined,
        draftValue: state.modal.draftValue ?? row.effectiveValueText,
        preview: undefined,
      },
    };
  }

  if (state.modal.phase === "write-result") {
    return {
      ...state,
      modal: {
        ...state.modal,
        phase: "browse",
        findQuery: undefined,
        message: undefined,
        preview: undefined,
      },
    };
  }

  return state;
}

function nextConfigTab(current: MissionControlConfigTab, delta: 1 | -1): MissionControlConfigTab {
  const tabs: MissionControlConfigTab[] = [
    "overview",
    "effective",
    "project",
    "global",
    "defaults",
    "workers",
    "plan",
    "doctor",
  ];
  const index = tabs.indexOf(current);
  if (index < 0) return "overview";
  return tabs[(index + delta + tabs.length) % tabs.length]!;
}

function cycleConfigDraft(state: AppState, direction: "previous" | "next"): AppState {
  if (state.modal.kind !== "config") return state;
    const row = getConfigRowsForTab(
      state.snapshot.configInspector ?? null,
      state.modal.tab,
      state.modal.findQuery,
    )[state.modal.selectedRowIndex];
    if (!row?.options || row.options.length === 0) return state;

  const currentValue = state.modal.draftValue ?? row.effectiveValueText;
  const currentIndex = Math.max(0, row.options.indexOf(currentValue));
  const nextIndex = direction === "next"
    ? (currentIndex + 1) % row.options.length
    : (currentIndex - 1 + row.options.length) % row.options.length;

    return {
      ...state,
      modal: {
        ...state.modal,
        draftValue: row.options[nextIndex],
        message: undefined,
        preview: undefined,
      },
    };
  }
