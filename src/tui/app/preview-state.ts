import { MaestroError } from "../../domain/errors.js";
import { createInitialState, reduce, type AppState } from "../state/reducer.js";
import type { MissionControlSnapshot } from "../state/types.js";

export const PREVIEW_SCREENS = [
  "dashboard",
  "features",
  "dependencies",
  "handoffs",
  "config",
  "runtime",
  "workers",
  "output",
] as const;

export type PreviewScreen = typeof PREVIEW_SCREENS[number];

export function isPreviewScreen(value: string): value is PreviewScreen {
  return PREVIEW_SCREENS.includes(value as PreviewScreen);
}

export interface PreviewSelectionOptions {
  screen?: PreviewScreen;
  featureId?: string;
  handoffId?: string;
}

export interface PreviewStateOptions extends PreviewSelectionOptions {
  snapshot: MissionControlSnapshot;
}

const FEATURE_SELECTOR_SCREENS: readonly PreviewScreen[] = [
  "dashboard",
  "features",
  "dependencies",
  "output",
];

export function buildPreviewState(opts: PreviewStateOptions): AppState {
  const screen = opts.screen ?? "dashboard";
  validateSelectorUsage(screen, opts);

  const state = createInitialState(opts.snapshot);
  const selectedFeatureIndex = resolveSelectedFeatureIndex(opts);
  const selectedHandoffIndex = resolveSelectedHandoffIndex(opts);

  const baseState = selectedFeatureIndex === undefined
    ? state
    : { ...state, selectedFeatureIndex };

  switch (screen) {
    case "dashboard":
      return opts.featureId
        ? { ...baseState, leftPaneMode: "preview" }
        : baseState;
    case "features":
      return reduce(baseState, { type: "open-features" });
    case "dependencies":
      if (opts.snapshot.mode !== "mission") {
        throw new MaestroError("Dependencies preview requires a mission", [
          "Run `maestro mission-control --preview` to view the home dashboard",
          "Run `maestro mission-control --preview features` to inspect home overview details",
        ]);
      }
      return reduce(baseState, { type: "open-dependencies" });
    case "handoffs": {
      const handoffState = reduce(baseState, { type: "open-handoffs" });
      if (handoffState.modal.kind !== "handoffs" || selectedHandoffIndex === undefined) {
        return handoffState;
      }
      return {
        ...handoffState,
        modal: { ...handoffState.modal, selectedHandoffIndex },
      };
    }
    case "config":
      return reduce(baseState, { type: "open-config" });
    case "runtime":
      if (opts.snapshot.mode === "mission") {
        return reduce(baseState, { type: "open-processes" });
      }
      return {
        ...baseState,
        modal: {
          kind: "processes",
          selectedProcessIndex: 0,
        },
      };
    case "workers":
      return reduce(baseState, { type: "open-workers" });
    case "output": {
      if (opts.snapshot.mode !== "mission") {
        throw new MaestroError("Runtime output preview requires a mission", [
          "Run `maestro mission-control --preview` to view the home dashboard",
        ]);
      }
      const processIndex = opts.featureId
        ? opts.snapshot.runtimeProcesses.findIndex((process) => process.featureId === opts.featureId)
        : 0;
      if (processIndex < 0) {
        throw new MaestroError(`Feature ${opts.featureId} does not have a live runtime output stream`, [
          `Run \`maestro mission-control --mission ${opts.snapshot.missionId} --preview runtime\` to inspect live items`,
        ]);
      }
      const withProcess = reduce(baseState, { type: "open-processes" });
      const nextState = withProcess.modal.kind === "processes"
        ? {
          ...withProcess,
          modal: {
            ...withProcess.modal,
            selectedProcessIndex: processIndex,
          },
        }
        : withProcess;
      return reduce(nextState, { type: "open-runtime-output" });
    }
  }
}

function validateSelectorUsage(screen: PreviewScreen, opts: PreviewStateOptions): void {
  if (opts.featureId && !FEATURE_SELECTOR_SCREENS.includes(screen)) {
    throw new MaestroError("--feature is only supported for dashboard, features, and dependencies previews", [
      "Try `maestro mission-control --preview dashboard --feature <id>`",
      "Try `maestro mission-control --preview dependencies --feature <id>`",
    ]);
  }

  if (opts.handoffId && screen !== "handoffs") {
    throw new MaestroError("--handoff is only supported for handoffs previews", [
      "Try `maestro mission-control --preview handoffs --handoff <id>`",
    ]);
  }
}

function resolveSelectedFeatureIndex(opts: PreviewStateOptions): number | undefined {
  if (!opts.featureId) return undefined;

  if (opts.snapshot.mode !== "mission") {
    throw new MaestroError("Feature previews require an active mission", [
      "Run `maestro mission-control --preview` for the home dashboard",
      "Omit `--feature` when previewing home mode",
    ]);
  }

  const featureIndex = opts.snapshot.features.findIndex((feature) => feature.id === opts.featureId);
  if (featureIndex >= 0) return featureIndex;

  throw new MaestroError(`Feature ${opts.featureId} not found in mission ${opts.snapshot.missionId}`, [
    `List tasks with \`maestro mission-control --mission ${opts.snapshot.missionId} --preview features\``,
  ]);
}

function resolveSelectedHandoffIndex(opts: PreviewStateOptions): number | undefined {
  if (!opts.handoffId) {
    return opts.snapshot.pendingHandoffs.length > 0 ? 0 : undefined;
  }

  const handoffIndex = opts.snapshot.pendingHandoffs.findIndex((handoff) => handoff.id === opts.handoffId);
  if (handoffIndex >= 0) return handoffIndex;

  throw new MaestroError(`Handoff ${opts.handoffId} not found in pending handoffs`, [
    "Run `maestro mission-control --preview handoffs` to list pending handoffs",
  ]);
}
