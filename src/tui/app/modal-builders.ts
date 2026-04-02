/**
 * Modal option builders and command palette glue.
 * Extracted from index.ts -- builds ModalOptions from AppState.
 */
import type { AppState, Action } from "../state/reducer.js";
import type { MissionControlConfigRow, MissionControlSnapshot, TaskPreviewPane } from "../state/types.js";
import {
  getFilteredMissionControlCommandSpecs,
  getMissionControlCommandSpecs,
  type MissionControlCommandId,
} from "../state/mission-control-commands.js";
import {
  buildOverlayRenderSpec,
  type ModalOptions,
} from "../widgets/modal.js";
import { getValidFeatureTransitions } from "../../domain/mission-state.js";
import { FEATURE_STATUS_LABEL, FEATURE_TASK_STATUS_LABEL } from "../theme.js";
import { shortenSessionId } from "../session-id.js";
import { getConfigRowsForTab } from "../state/config-inspector.js";

export function buildModalOptions(state: AppState): ModalOptions | undefined {
  if (state.modal.kind === "command-palette") {
    const commands = getFilteredCommandPaletteItems(state);
    return {
      mode: "palette",
      title: "Command Palette",
      query: state.modal.query,
      items: commands.map((command) => ({
        label: command.label,
        detail: command.detail,
        hint: command.hint,
        section: command.section,
      })),
      selectedIndex: Math.min(
        state.modal.selectedCommandIndex,
        Math.max(0, commands.length - 1),
      ),
      emptyLabel: "No commands match your filter",
      renderSpec: buildOverlayRenderSpec("command-palette"),
    };
  }

  if (state.modal.kind === "feature-action") {
    const feature = state.snapshot.features[state.modal.featureIndex];
    if (!feature) return undefined;

    const transitions = getValidFeatureTransitions(feature.status);
      return {
        mode: "menu",
        title: "Change Feature Status",
        eyebrow: `${feature.id} · ${feature.title}`,
      items: transitions.length > 0
        ? transitions.map((transition) => ({
          label: `Set status to ${transition}`,
          detail: `Move ${feature.id} from ${FEATURE_STATUS_LABEL[feature.status]} to ${transition}`,
          section: "Transitions",
        }))
          : [{ label: "No valid transitions", detail: "This feature cannot move to another state right now.", section: "Transitions", tone: "muted" }],
        selectedIndex: state.modal.selectedOption,
        footer: getFeatureActionFooter(state.modal),
        renderSpec: buildOverlayRenderSpec("feature-action"),
      };
    }

  if (state.modal.kind === "feature-browser") {
      return {
        mode: "menu",
        title: "Tasks",
        eyebrow: state.snapshot.mode === "home" ? "Project overview" : "Select a task to focus",
      items: state.snapshot.features.length > 0
        ? state.snapshot.features.map((feature) => ({
          label: feature.title,
          detail: `${feature.id} · ${FEATURE_STATUS_LABEL[feature.status]} · ${feature.workerType}`,
          hint: feature.hasReport ? "report" : undefined,
          section: "Mission",
        }))
        : [{ label: "No features available", detail: "This mission does not have any features yet.", section: "Mission", tone: "muted" }],
        selectedIndex: Math.min(
          state.modal.selectedFeatureIndex,
          Math.max(0, state.snapshot.features.length - 1),
        ),
        footer: state.modal.returnTarget === "command-palette"
          ? "Enter focus · Left back · Esc close"
          : "Enter focus · Esc close",
        renderSpec: buildOverlayRenderSpec("feature-browser"),
      };
    }

    if (state.modal.kind === "dependencies") {
      const preview = getSelectedTaskPreview(state);
      return {
        mode: "split",
        title: "Dependencies",
        eyebrow: preview?.title ?? "No task selected",
        items: preview
          ? buildDependencyListItems(preview)
          : buildEmptyDependencyListItems(),
        selectedIndex: Math.min(
          state.modal.selectedOption,
          Math.max(0, ((preview?.blockedBy?.length ?? 0) + (preview?.unblocks?.length ?? 0)) - 1),
        ),
        detailItems: preview
          ? buildDependencyDetailItems(preview)
          : [{ text: "No dependency graph available", tone: "muted" as const }],
        footer: buildOverlayFooter(state.modal.returnTarget, "Enter jump"),
        renderSpec: buildOverlayRenderSpec("dependencies"),
      };
    }

  if (state.modal.kind === "overview" && state.snapshot.home) {
      return {
        mode: "info",
        title: "Overview",
        eyebrow: state.snapshot.home.headline,
      items: [
        { text: state.snapshot.home.summary, section: "Environment" },
        { text: state.snapshot.home.locationLabel, style: "block", tone: "accent", section: "Location" },
        ...state.snapshot.home.actions.map((action) => ({
          text: action.command,
          detail: action.detail,
          hint: action.label,
          section: "Next Steps",
          tone: "muted" as const,
          })),
        ],
        footer: state.modal.returnTarget === "command-palette" ? "Left back · Esc close" : "Esc close",
        renderSpec: buildOverlayRenderSpec("overview"),
      };
    }

    if (state.modal.kind === "handoffs") {
      const items = state.snapshot.pendingHandoffs.map((handoff) => ({
        label: `${handoff.id} · ${handoff.agent}`,
      }));
      const selectedHandoff = state.snapshot.pendingHandoffs[state.modal.selectedHandoffIndex];
      return {
        mode: "split",
        title: "Handoffs",
        eyebrow: state.snapshot.pendingHandoffs.length > 0
          ? `${state.snapshot.pendingHandoffs.length} pending`
          : "No pending handoffs",
        items: items.length > 0
          ? items
          : [{ label: "No pending handoffs in this workspace.", selectable: false, tone: "muted" }],
        selectedIndex: Math.min(state.modal.selectedHandoffIndex, Math.max(0, items.length - 1)),
        detailItems: buildHandoffDetailItems(selectedHandoff),
        footer: buildOverlayFooter(state.modal.returnTarget, "Enter inspect"),
        renderSpec: buildOverlayRenderSpec("handoffs"),
      };
    }

    if (state.modal.kind === "config") {
      const rows = getConfigRowsForTab(state.snapshot.configInspector ?? null, state.modal.tab);
      const selectedRow = rows[state.modal.selectedRowIndex];
      return {
        mode: "split",
        title: "Config",
        eyebrow: buildConfigTabs(state),
        items: rows.length > 0
          ? rows.map((row) => ({
              label: row.label,
              detail: `${row.valueText} · ${row.source}`,
              section: row.section,
            }))
          : [{ label: "No config rows available", selectable: false, tone: "muted" }],
        selectedIndex: Math.min(state.modal.selectedRowIndex, Math.max(0, rows.length - 1)),
        detailItems: buildConfigDetailItems(state, selectedRow),
        footer: buildConfigFooter(state),
        renderSpec: buildOverlayRenderSpec("config"),
      };
    }

    if (state.modal.kind === "processes") {
      const items = state.snapshot.runtimeProcesses.map((process) => ({
        label: `${process.featureId} · ${process.title}`,
      }));
      const selectedProcess = state.snapshot.runtimeProcesses[state.modal.selectedProcessIndex];
      return {
        mode: "split",
        title: "Runtime",
        eyebrow: state.snapshot.runtimeProcesses.length > 0
          ? `${state.snapshot.runtimeProcesses.length} runtime item${state.snapshot.runtimeProcesses.length === 1 ? "" : "s"}`
          : "No active runtime processes",
        items: items.length > 0
          ? items
          : [{ label: "No assigned, in-progress, or review features right now.", selectable: false, tone: "muted" }],
        selectedIndex: Math.min(state.modal.selectedProcessIndex, Math.max(0, items.length - 1)),
        detailItems: buildRuntimeDetailItems(selectedProcess),
        footer: buildOverlayFooter(state.modal.returnTarget, "Enter inspect"),
        renderSpec: buildOverlayRenderSpec("processes"),
      };
    }

  return undefined;
}

function getFeatureActionFooter(modal: Extract<AppState["modal"], { kind: "feature-action" }>): string {
  if (modal.phase === "submitting") {
    return "Applying status...";
  }
  if (modal.phase === "error") {
    return `${modal.errorMessage ?? "Failed to update feature"} · Enter retry · Esc cancel`;
  }
  if (modal.phase === "confirming") {
    return "Enter confirm · Esc cancel";
  }
  return "Use arrows or click · Enter choose · Esc cancel";
}

function buildOverlayFooter(returnTarget: "command-palette" | undefined, enterLabel: string): string {
  return returnTarget === "command-palette"
    ? `${enterLabel} · Left back · Esc close`
    : `${enterLabel} · Esc close`;
}

function buildConfigTabs(state: AppState): string {
  if (state.modal.kind !== "config") return "";
  const tabs = state.snapshot.configInspector?.tabs ?? [];
  return tabs
    .map((tab) => tab === state.modal.tab ? `[${tab}]` : tab)
    .join(" ");
}

function buildConfigDetailItems(
  state: AppState,
  row: MissionControlConfigRow | undefined,
) {
  if (state.modal.kind !== "config") return [];

  const items = [
    {
      text: `target scope: ${state.modal.selectedScope}`,
      section: "Write Scope",
      tone: "accent" as const,
    },
  ];

  if (!row) {
    items.push({
      text: "No config rows available for this tab",
      section: "Details",
      tone: "muted" as const,
    });
    return items;
  }

  items.push(
    { text: row.description, section: "Meaning" },
    { text: `effective: ${row.effectiveValueText}`, section: "Values" },
  );

  if (row.projectValueText !== undefined) {
    items.push({ text: `project: ${row.projectValueText}`, section: "Values", tone: "muted" as const });
  }
  if (row.globalValueText !== undefined) {
    items.push({ text: `global: ${row.globalValueText}`, section: "Values", tone: "muted" as const });
  }
  if (row.defaultValueText !== undefined) {
    items.push({ text: `default: ${row.defaultValueText}`, section: "Values", tone: "muted" as const });
  }

  if (state.modal.phase === "choose-scope") {
    items.push({
      text: `choose project/global, current: ${state.modal.selectedScope}`,
      section: "Edit",
      tone: "accent" as const,
    });
  }

  if (state.modal.phase === "edit-inline") {
    items.push({
      text: `draft: ${state.modal.draftValue ?? row.effectiveValueText}`,
      section: "Edit",
      tone: "accent" as const,
    });
    if (row.options?.length) {
      items.push({
        text: `options: ${row.options.join(" | ")}`,
        section: "Edit",
      });
    }
  }

  if (state.modal.phase === "confirm-write") {
    items.push({
      text: `apply ${row.keyPath} = ${state.modal.draftValue ?? row.effectiveValueText}`,
      section: "Confirm",
      tone: "accent" as const,
    });
  }

  if (state.modal.phase === "write-result" && state.modal.message) {
    items.push({
      text: state.modal.message,
      section: "Result",
      tone: "accent" as const,
    });
  }

  if (state.snapshot.configInspector?.errors.length) {
    items.push({
      text: `errors: ${state.snapshot.configInspector.errors.join(" | ")}`,
      section: "Doctor",
      tone: "muted" as const,
    });
  }

  return items;
}

function buildConfigFooter(state: AppState): string {
  if (state.modal.kind !== "config") return "Esc close";
  switch (state.modal.phase) {
    case "choose-scope":
      return "S scope · Enter choose · Esc cancel";
    case "edit-inline":
      return "Left/Right change · S scope · Enter review · Esc cancel";
    case "confirm-write":
      return "Enter apply · Esc cancel";
    case "write-result":
      return "Enter acknowledge · Esc close";
    case "browse":
    default:
      return "[ / ] tabs · Up/Down rows · Enter edit · Esc close";
  }
}

function getSelectedTaskPreview(state: AppState): TaskPreviewPane | null {
  return state.snapshot.taskPreviews?.[state.selectedFeatureIndex] ?? state.snapshot.activeFeature ?? null;
}

function buildDependencyListItems(preview: TaskPreviewPane) {
  const blockedBy = preview.blockedBy ?? [];
  const unblocks = preview.unblocks ?? [];
  return [
    ...(blockedBy.length > 0
      ? blockedBy.flatMap((feature, index) => [
        {
          label: `${index + 1}. ${feature.id} ${feature.title}`,
          section: "Upstream",
        },
        {
          label: `status: ${formatTaskStatus(feature.status)}`,
          selectable: false,
          tone: "muted" as const,
        },
      ])
      : [{ label: "none", section: "Upstream", selectable: false, tone: "muted" as const }]),
    ...(unblocks.length > 0
      ? unblocks.flatMap((feature, index) => [
        {
          label: `${index + 1}. ${feature.id} ${feature.title}`,
          section: "Downstream",
        },
        {
          label: `status: ${formatTaskStatus(feature.status)}`,
          selectable: false,
          tone: "muted" as const,
        },
      ])
      : [{ label: "none", section: "Downstream", selectable: false, tone: "muted" as const }]),
    {
      label: `blocked by: ${blockedBy.length} ${blockedBy.length === 1 ? "dependency" : "dependencies"}`,
      section: "Summary",
      selectable: false,
      tone: "muted" as const,
    },
    {
      label: `ready to start: ${blockedBy.length === 0 ? "yes" : "no"}`,
      selectable: false,
      tone: "muted" as const,
    },
  ];
}

function buildEmptyDependencyListItems() {
  return [
    { label: "none", section: "Upstream", selectable: false, tone: "muted" as const },
    { label: "none", section: "Downstream", selectable: false, tone: "muted" as const },
    { label: "blocked by: 0 dependencies", section: "Summary", selectable: false, tone: "muted" as const },
    { label: "ready to start: no", selectable: false, tone: "muted" as const },
  ];
}

function buildDependencyDetailItems(preview: TaskPreviewPane) {
  const blockedBy = preview.blockedBy ?? [];
  const unblocks = preview.unblocks ?? [];
  const blockedByLines = blockedBy.map((feature, index) => {
    const isLast = index === blockedBy.length - 1 && unblocks.length === 0;
    return `${isLast ? "\u2514" : "\u251C"}\u2500 blocked by ${feature.id} [${FEATURE_TASK_STATUS_LABEL[feature.status]}]`;
  });
  const unblocksLines = unblocks.map((feature, index) => {
    const isLast = index === unblocks.length - 1;
    return `${isLast ? "\u2514" : "\u251C"}\u2500 unblocks ${feature.id} [${FEATURE_TASK_STATUS_LABEL[feature.status]}]`;
  });
  const graphLines = [
    `${preview.id} ${preview.title} [${FEATURE_TASK_STATUS_LABEL[preview.status]}]`,
    ...(blockedByLines.length > 0 || unblocksLines.length > 0
      ? [...blockedByLines, ...unblocksLines]
      : ["\u2514\u2500 ready to start [CLEAR]"]),
  ];
  return [
    { text: "Graph", section: "Graph", tone: "accent" as const },
    ...graphLines.map((line) => ({ text: line })),
  ];
}

function buildHandoffDetailItems(handoff: MissionControlSnapshot["pendingHandoffs"][number] | undefined) {
  if (!handoff) {
    return [{ text: "No pending handoff selected", tone: "muted" as const }];
  }

  return [
    { text: `${handoff.agent} handoff`, tone: "accent" as const, style: "block" as const },
    { text: handoff.message, section: "message" },
    { text: handoff.sessionId ? `${handoff.agent} · ${shortenSessionId(handoff.sessionId)}` : handoff.agent, section: "session" },
    ...(handoff.sitrep ? [{ text: handoff.sitrep, section: "sitrep" as const }] : []),
    ...(handoff.quickstart ? [{ text: handoff.quickstart, section: "quickstart" as const }] : []),
  ];
}

function buildRuntimeDetailItems(process: MissionControlSnapshot["runtimeProcesses"][number] | undefined) {
  if (!process) {
    return [{ text: "No runtime item selected", tone: "muted" as const }];
  }

  return [
    { text: process.title, tone: "accent" as const, style: "block" as const },
    { text: "agent", detail: process.agent ?? "unknown" },
    { text: "session", detail: process.sessionId ? shortenSessionId(process.sessionId) : "none" },
    { text: "worker", detail: process.workerType },
    { text: "runtime", detail: process.runtimeState ?? (process.isLive ? "live" : FEATURE_STATUS_LABEL[process.status]) },
    ...(typeof process.lastSeenAgeMs === "number" ? [{ text: "last seen", detail: `${Math.round(process.lastSeenAgeMs / 1000)}s ago` }] : []),
    ...(typeof process.retryCount === "number" ? [{ text: "retry", detail: String(process.retryCount) }] : []),
    ...(process.milestoneTitle ? [{ text: "milestone", detail: process.milestoneTitle }] : []),
    ...(process.profile ? [{ text: "profile", detail: process.profile }] : []),
    ...(process.failureReason ? [{ text: "failure", detail: process.failureReason }] : []),
  ];
}

function formatTaskStatus(status: keyof typeof FEATURE_TASK_STATUS_LABEL): string {
  return FEATURE_TASK_STATUS_LABEL[status].toLowerCase();
}

export function isSelectableListModal(kind: AppState["modal"]["kind"]): kind is "feature-browser" | "handoffs" | "processes" | "dependencies" {
  return kind === "feature-browser"
    || kind === "handoffs"
    || kind === "processes"
    || kind === "dependencies";
}

interface CommandPaletteItem {
  readonly id: MissionControlCommandId;
  readonly label: string;
  readonly detail: string;
  readonly hint: string;
  readonly section: string;
  readonly keywords: readonly string[];
  readonly action: Action;
}

function getCommandPaletteItems(state: AppState): readonly CommandPaletteItem[] {
  return getMissionControlCommandSpecs(state.snapshot.mode).map((command) => ({
    id: command.id,
    label: command.label,
    detail: command.detail,
    hint: command.key,
    section: command.section,
    keywords: command.keywords,
    action: actionForMissionControlCommand(command.id),
  }));
}

export function getFilteredCommandPaletteItems(state: AppState): readonly CommandPaletteItem[] {
  if (state.modal.kind !== "command-palette") return [];

  const filteredCommands = getFilteredMissionControlCommandSpecs(
    state.snapshot.mode,
    state.modal.query,
  );
  const itemsById = new Map(getCommandPaletteItems(state).map((item) => [item.id, item]));
  return filteredCommands
    .map((command) => itemsById.get(command.id))
    .filter((item): item is CommandPaletteItem => item !== undefined);
}

export function getCommandPaletteSelectionAction(state: AppState): Action | undefined {
  if (state.modal.kind !== "command-palette") return undefined;

  const commands = getFilteredCommandPaletteItems(state);
  if (commands.length === 0) return undefined;

  const index = Math.min(state.modal.selectedCommandIndex, commands.length - 1);
  return commands[index]?.action;
}

export function actionForMissionControlCommand(id: MissionControlCommandId): Action {
  switch (id) {
    case "features":
      return { type: "open-features" };
    case "dependencies":
      return { type: "open-dependencies" };
    case "handoffs":
      return { type: "open-handoffs" };
    case "config":
      return { type: "open-config" };
    case "processes":
      return { type: "open-processes" };
    case "exit":
      return { type: "quit" };
  }
}
