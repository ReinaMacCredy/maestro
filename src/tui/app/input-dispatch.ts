/**
 * Key-to-action mapping and input dispatch helpers.
 * Extracted from index.ts -- pure functions, no side effects.
 */
import type { Key } from "../terminal/input.js";
import type { AppState, Action } from "../state/reducer.js";
import { getMissionControlCommandSpecs } from "../state/mission-control-commands.js";
import { actionForMissionControlCommand } from "./modal-builders.js";

export function keyToAction(key: Key, state: AppState): Action | undefined {
  if (key.type === "char" && key.char === "q" && state.modal.kind === "none") {
    return { type: "quit" };
  }
  if (key.type === "ctrl" && (key.char === "t" || key.char === "c")) {
    return { type: "quit" };
  }
  if (key.type === "ctrl" && key.char === "p") {
    return { type: "open-command-palette" };
  }
  if (key.type === "ctrl" && key.char === "y") {
    return { type: "toggle-copy-mode" };
  }
    if (key.type === "escape") {
      return { type: "escape" };
    }
    if (state.modal.kind === "config" && state.modal.phase === "browse") {
      if (key.type === "char" && key.char === "/") {
        return { type: "config-find-start" };
      }
      if (state.modal.findQuery !== undefined) {
        if (key.type === "char") {
          return { type: "config-find-append", char: key.char };
        }
        if (key.type === "backspace" || key.type === "delete") {
          return { type: "config-find-backspace" };
        }
      }
    }
    if (state.modal.kind === "config" && state.modal.phase === "edit-inline" && key.type === "arrow") {
      if (key.direction === "left") {
        return { type: "config-cycle-value", direction: "previous" };
      }
    if (key.direction === "right") {
      return { type: "config-cycle-value", direction: "next" };
    }
  }
    if (
      key.type === "arrow"
      && key.direction === "left"
      && (
        ((
          state.modal.kind === "feature-browser"
          || state.modal.kind === "dependencies"
          || state.modal.kind === "overview"
        || state.modal.kind === "handoffs"
        || state.modal.kind === "processes"
      ) && state.modal.returnTarget === "command-palette")
    )
  ) {
    return { type: "navigate", direction: "left" };
  }
  if (key.type === "arrow" && (key.direction === "up" || key.direction === "down")) {
    return { type: "navigate", direction: key.direction };
  }
  if ((key.type === "backspace" || key.type === "delete") && state.modal.kind === "command-palette") {
    return { type: "modal-query-backspace" };
  }
  if (key.type === "enter") {
    return { type: "enter" };
  }
    if (key.type === "char" && state.modal.kind === "config") {
      switch (key.char) {
        case "[":
          return { type: "config-prev-tab" };
        case "]":
          return { type: "config-next-tab" };
        case "s":
        case "S":
          return { type: "config-toggle-scope" };
      }
    }
  if (key.type === "char" && state.modal.kind === "command-palette") {
    return { type: "modal-query-append", char: key.char };
  }
  if (key.type === "char" && state.modal.kind === "none") {
    const hotkey = key.char.toUpperCase();
    const command = getMissionControlCommandSpecs(state.snapshot.mode)
      .find((spec) => spec.key === hotkey);
    if (command) {
      return actionForMissionControlCommand(command.id);
    }
    switch (hotkey) {
      case "L":
      case "W":
        return { type: "focus", panel: "log" };
    }
  }
  return undefined;
}

export function shouldSubmitFeatureAction(state: AppState): boolean {
  return state.modal.kind === "feature-action"
    && (state.modal.phase === "confirming" || state.modal.phase === "error");
}
