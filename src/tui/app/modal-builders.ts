/**
 * Modal option builders and command palette glue.
 * Extracted from index.ts -- builds ModalOptions from AppState.
 */
import type { AppState, Action } from "../state/reducer.js";
import type {
  MissionControlConfigInspector,
  MissionControlConfigRow,
  MissionControlSnapshot,
  TaskPreviewPane,
} from "../state/types.js";
import {
  getFilteredMissionControlCommandSpecs,
  getMissionControlCommandSpecs,
  type MissionControlCommandId,
} from "../state/mission-control-commands.js";
import {
  buildOverlayRenderSpec,
  type ModalInfoItem,
  type ModalOptions,
} from "../shared/modal-model.js";
import { truncate } from "../shared/format.js";
import { getValidFeatureTransitions } from "@/shared/domain/legacy-mission";
import { TASK_STATUSES } from "@/shared/domain/task";
import type { TaskBoardItem } from "../state/screen-types.js";
import { FEATURE_STATUS_LABEL, FEATURE_TASK_STATUS_LABEL, TASK_STATUS_COLUMN_LABEL, AGENT_STATUS_LABEL } from "../shared/theme.js";
  import {
    getConfigRowsForTab,
    getConfigTabDisplayLabel,
    isGlobalOnlyConfigKey,
    resolveConfigScopeForKey,
  } from "../state/config-inspector.js";
function formatAgentLabel(slug: string): string {
  return slug
    .split("-")
    .map((part) => part.length === 0 ? part : part[0]!.toUpperCase() + part.slice(1))
    .join(" ");
}

type MemoryModalState = AppState & {
  modal: Extract<AppState["modal"], { kind: "memory" }>;
};

type GraphModalState = AppState & {
  modal: Extract<AppState["modal"], { kind: "graph" }>;
};

type ConfigModalState = AppState & {
  modal: Extract<AppState["modal"], { kind: "config" }>;
};

export function buildModalOptions(state: AppState): ModalOptions | undefined {
  const returnTarget = "returnTarget" in state.modal ? state.modal.returnTarget : undefined;
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
          detail: `${feature.id} · ${FEATURE_STATUS_LABEL[feature.status]} · ${feature.agentType}`,
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
          returnTarget,
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
          returnTarget,
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
          returnTarget,
          renderSpec: buildOverlayRenderSpec("overview"),
        };
    }

      if (state.modal.kind === "config") {
        const rows = getConfigRowsForTab(
          state.snapshot.configInspector ?? null,
          state.modal.tab,
          state.modal.findQuery,
        );
        const selectedRow = rows[state.modal.selectedRowIndex];
        const configState = state as ConfigModalState;
        const configItems = buildConfigItems(configState, rows, selectedRow);
        if (state.modal.phase === "write-result") {
          return {
            mode: "info",
            title: "Change Saved",
              eyebrow: selectedRow?.label,
              items: buildConfigResultItems(state, selectedRow),
              returnTarget,
              renderSpec: buildOverlayRenderSpec("config"),
            };
          }
          return {
          mode: "split",
          title: buildConfigTitle(state),
            eyebrow: buildConfigEyebrow(state),
            listTitle: buildConfigListTitle(state),
            detailTitle: buildConfigDetailTitle(state),
            items: configItems.items,
            selectedIndex: configItems.selectedIndex,
            detailItems: buildConfigDetailItems(configState, selectedRow),
            returnTarget,
            renderSpec: buildOverlayRenderSpec("config"),
          };
          }

      if (state.modal.kind === "memory") {
        return buildMemoryModal(state as MemoryModalState, returnTarget);
      }

      if (state.modal.kind === "graph") {
        return buildGraphModal(state as GraphModalState, returnTarget);
      }

      if (state.modal.kind === "agent-grid") {
        return buildAgentGridModal(state as AgentGridModalState, returnTarget);
      }

      if (state.modal.kind === "dispatch") {
        return buildDispatchModal(state as DispatchModalState, returnTarget);
      }

      if (state.modal.kind === "event-stream") {
        return buildEventStreamModal(state as EventStreamModalState, returnTarget);
      }

      if (state.modal.kind === "task-board") {
        return buildTaskBoardModal(state as TaskBoardModalState, returnTarget);
      }

      if (state.modal.kind === "timeline") {
        return buildTimelineModal(state as TimelineModalState, returnTarget);
      }

      if (state.modal.kind === "principle-review") {
        return buildPrincipleReviewModal(state as PrincipleReviewModalState, returnTarget);
      }

      if (state.modal.kind === "help") {
        return buildHelpModal(state, returnTarget);
      }

      if (state.modal.kind === "autopilot") {
        return buildAutopilotModal(state as AutopilotModalState, returnTarget);
      }

  return undefined;
}

function buildMemoryModal(
  state: MemoryModalState,
  returnTarget: "command-palette" | undefined,
): ModalOptions {
  // Memory/graph subsystems were retired; durable rules live under `maestro principle`.
  void state;
  return {
    mode: "info",
    title: "Memory",
    eyebrow: "Retired · see `maestro principle` for the durable-rule surface.",
    items: [
        {
          text: "The earlier memory subsystem (corrections, lessons, ratchet) was retired.",
          tone: "muted" as const,
        },
      { text: "Promoted principles live under `maestro principle list`." },
    ],
    footer: buildTabbedOverlayFooter(returnTarget),
    returnTarget,
    renderSpec: buildOverlayRenderSpec("memory"),
  };
}

function buildGraphModal(
  state: GraphModalState,
  returnTarget: "command-palette" | undefined,
): ModalOptions {
  void state;
  return {
    mode: "info",
    title: "Project Graph",
    eyebrow: "Retired · project-graph context is no longer surfaced.",
    items: [
      {
        text: "Cross-project graph data was retired with the earlier memory subsystem.",
        tone: "muted" as const,
      },
    ],
    footer: buildTabbedOverlayFooter(returnTarget),
    returnTarget,
    renderSpec: buildOverlayRenderSpec("graph"),
  };
}

// ---------------------------------------------------------------------------
// Conductor screen modal builders
// ---------------------------------------------------------------------------

type AgentGridModalState = AppState & { modal: Extract<AppState["modal"], { kind: "agent-grid" }> };
type DispatchModalState = AppState & { modal: Extract<AppState["modal"], { kind: "dispatch" }> };
type EventStreamModalState = AppState & { modal: Extract<AppState["modal"], { kind: "event-stream" }> };
type TaskBoardModalState = AppState & { modal: Extract<AppState["modal"], { kind: "task-board" }> };
type TimelineModalState = AppState & { modal: Extract<AppState["modal"], { kind: "timeline" }> };
type PrincipleReviewModalState = AppState & { modal: Extract<AppState["modal"], { kind: "principle-review" }> };
type AutopilotModalState = AppState & { modal: Extract<AppState["modal"], { kind: "autopilot" }> };

const TASK_TABLE_WIDTH = 76;
const TASK_TABLE_STATUS_WIDTH = 4;
const TASK_TABLE_PRIORITY_WIDTH = 3;
const TASK_TABLE_ID_WIDTH = 6;
const TASK_TABLE_EVIDENCE_WIDTH = 2;
const TASK_TABLE_BLOCKER_WIDTH = 2;
const TASK_TABLE_TITLE_WIDTH = TASK_TABLE_WIDTH
  - TASK_TABLE_STATUS_WIDTH
  - TASK_TABLE_PRIORITY_WIDTH
  - TASK_TABLE_ID_WIDTH
  - TASK_TABLE_EVIDENCE_WIDTH
  - TASK_TABLE_BLOCKER_WIDTH
  - 5;
const DISPATCH_TABLE_STATUS_WIDTH = 4;
const DISPATCH_TABLE_TITLE_WIDTH = 48;
const DISPATCH_TABLE_ID_WIDTH = 6;
const DISPATCH_TABLE_AGENT_WIDTH = 10;

function buildAgentGridModal(
  state: AgentGridModalState,
  returnTarget: "command-palette" | undefined,
): ModalOptions {
  const grid = state.snapshot.agentGrid ?? [];
  const selected = grid[state.modal.selectedIndex];
  return {
    mode: "split",
    title: "Agent Grid",
    eyebrow: "Agent status and feature assignments.",
    items: grid.length > 0
      ? grid.map((row) => ({
          label: formatAgentLabel(row.agentType),
          detail: AGENT_STATUS_LABEL[row.status],
          hint: `${row.completedCount}/${row.featureCount}`,
        }))
      : [{ label: "No agents assigned", selectable: false, tone: "muted" as const }],
    selectedIndex: Math.min(state.modal.selectedIndex, Math.max(0, grid.length - 1)),
    detailItems: selected
      ? [
          { text: `Agent: ${formatAgentLabel(selected.agentType)}` },
          { text: `Status: ${AGENT_STATUS_LABEL[selected.status]}` },
          ...(selected.activeFeatureId
            ? [{ text: `Active: ${selected.activeFeatureId}`, detail: selected.activeFeatureTitle }]
            : []),
          { text: `Progress: ${selected.completedCount}/${selected.featureCount} features done` },
          ...(selected.lastActivityAt
            ? [{ text: `Last activity: ${new Date(selected.lastActivityAt).toLocaleString()}`, tone: "muted" as const }]
            : []),
        ]
      : [{ text: "Select an agent to view details" }],
    footer: buildListOverlayFooter(returnTarget),
    returnTarget,
    renderSpec: buildOverlayRenderSpec("agent-grid"),
  };
}

function buildDispatchModal(
  state: DispatchModalState,
  returnTarget: "command-palette" | undefined,
): ModalOptions {
  const queue = state.snapshot.dispatchQueue ?? [];
  const selected = queue[state.modal.selectedIndex];
  const { phase } = state.modal;

  let footer = buildOverlayFooter(returnTarget, "prepare");
  if (phase === "generating") footer = "Generating agent prompt...";
  else if (phase === "generated") footer = `Prompt written to ${state.modal.promptPath ?? "disk"} -- Esc close`;
  else if (phase === "error") footer = `Error: ${state.modal.errorMessage ?? "unknown"} -- Esc close`;

  return {
    mode: "split",
    title: "Dispatch Console",
    eyebrow: phase === "browse"
      ? "Ready features sorted by milestone priority."
      : phase === "generating"
        ? "Generating..."
        : phase === "generated"
          ? "Prompt generated."
          : "Error during generation.",
    solidPanels: true,
    stackedPanels: true,
    items: queue.length > 0
      ? [
          {
            label: formatDispatchTableHeader(),
            selectable: false,
            tone: "muted" as const,
          },
          ...queue.map((item) => ({
            label: formatDispatchTableRow(item),
          })),
        ]
      : [{ label: "No features ready for dispatch", selectable: false, tone: "muted" as const }],
    selectedIndex: queue.length > 0
      ? Math.min(state.modal.selectedIndex + 1, queue.length)
      : 0,
    detailItems: selected
      ? [
          ...wrapTaskDetailText(selected.featureTitle).map((text) => ({ text })),
          { text: `ID: ${selected.featureId}` },
          { text: `Milestone: ${selected.milestoneTitle} (#${selected.milestoneOrder})` },
          { text: `Agent: ${formatAgentLabel(selected.agentType)}` },
        ]
      : [{ text: "Select a feature to view dispatch details" }],
    footer,
    returnTarget,
    renderSpec: buildOverlayRenderSpec("dispatch"),
  };
}

function formatDispatchTableHeader(): string {
  return [
    padTaskCell("ST", DISPATCH_TABLE_STATUS_WIDTH),
    padTaskCell("TASK", DISPATCH_TABLE_TITLE_WIDTH),
    padTaskCell("ID", DISPATCH_TABLE_ID_WIDTH),
    padTaskCell("AGENT", DISPATCH_TABLE_AGENT_WIDTH),
  ].join(" ");
}

function formatDispatchTableRow(item: NonNullable<AppState["snapshot"]["dispatchQueue"]>[number]): string {
  const shortId = item.featureId.split("-").at(-1) ?? item.featureId;
  return [
    padTaskCell("RDY", DISPATCH_TABLE_STATUS_WIDTH),
    padTaskCell(item.featureTitle, DISPATCH_TABLE_TITLE_WIDTH),
    padTaskCell(shortId, DISPATCH_TABLE_ID_WIDTH),
    padTaskCell(formatAgentLabel(item.agentType), DISPATCH_TABLE_AGENT_WIDTH),
  ].join(" ");
}

function buildEventStreamModal(
  state: EventStreamModalState,
  returnTarget: "command-palette" | undefined,
): ModalOptions {
  const allEvents = state.snapshot.eventStream ?? [];
  const filterKind = state.modal.filterKind;
  const events = filterKind ? allEvents.filter((e) => e.kind === filterKind) : allEvents;
  const selected = events[state.modal.selectedIndex];
  const filterLabel = filterKind ?? "all";

  return {
    mode: "split",
    title: "Event Stream",
    eyebrow: `Showing: ${filterLabel} (F to cycle filter) -- ${events.length} events`,
    items: events.length > 0
      ? events.map((e) => ({
          label: e.title,
          detail: e.kind,
          hint: new Date(e.timestamp).toLocaleTimeString(),
        }))
      : [{ label: "No events", selectable: false, tone: "muted" as const }],
    selectedIndex: Math.min(state.modal.selectedIndex, Math.max(0, events.length - 1)),
    detailItems: selected
      ? [
          { text: selected.title },
          { text: `Kind: ${selected.kind}` },
          { text: `Time: ${new Date(selected.timestamp).toLocaleString()}` },
          ...(selected.detail ? [{ text: selected.detail, tone: "muted" as const }] : []),
        ]
      : [{ text: "Select an event to view details" }],
    footer: buildListOverlayFooter(returnTarget),
    returnTarget,
    renderSpec: buildOverlayRenderSpec("event-stream"),
  };
}

function buildTaskBoardModal(
  state: TaskBoardModalState,
  returnTarget: "command-palette" | undefined,
): ModalOptions {
  const board = state.snapshot.taskBoard;
  const col = state.modal.selectedColumn;
  const items = board?.columns[col] ?? [];
  const selected = items[state.modal.selectedIndex];

  const columnTabs = TASK_STATUSES
    .map((s) => {
      const count = board?.columns[s]?.length ?? 0;
      const label = `${TASK_STATUS_COLUMN_LABEL[s]} (${count})`;
      return s === col ? `[${label}]` : label;
    })
    .join("  ");

  return {
    mode: "split",
    title: "Task Board",
    eyebrow: columnTabs,
    listTitle: TASK_STATUS_COLUMN_LABEL[col],
    solidPanels: true,
    stackedPanels: true,
    items: items.length > 0
      ? [
          {
            label: formatTaskTableHeader(),
            selectable: false,
            tone: "muted" as const,
          },
          ...items.map((item) => ({
            label: formatTaskTableRow(item),
          })),
        ]
      : [{
          label: `No ${TASK_STATUS_COLUMN_LABEL[col].toLowerCase()} tasks`,
          selectable: false,
          tone: "muted" as const,
        }],
    selectedIndex: items.length > 0
      ? Math.min(state.modal.selectedIndex + 1, items.length)
      : 0,
    detailItems: selected
      ? [
          ...wrapTaskDetailText(selected.title).map((text) => ({ text })),
          { text: `ID: ${selected.id}` },
          { text: `Priority: ${selected.priority}  Evidence: ${selected.evidenceCount}  Blockers: ${selected.blockedByCount}` },
          ...(selected.assignee ? [{ text: `Assignee: ${selected.assignee}` }] : []),
        ]
      : [{ text: "Select a task to view details" }],
    footer: `Tab/[/] columns -- ${buildListOverlayFooter(returnTarget)}`,
    returnTarget,
    renderSpec: buildOverlayRenderSpec("task-board"),
  };
}

function formatTaskTableHeader(): string {
  return [
    padTaskCell("ST", TASK_TABLE_STATUS_WIDTH),
    padTaskCell("PRI", TASK_TABLE_PRIORITY_WIDTH),
    padTaskCell("TASK", TASK_TABLE_TITLE_WIDTH),
    padTaskCell("ID", TASK_TABLE_ID_WIDTH),
    padTaskCell("EV", TASK_TABLE_EVIDENCE_WIDTH),
    padTaskCell("BL", TASK_TABLE_BLOCKER_WIDTH),
  ].join(" ");
}

function formatTaskTableRow(item: TaskBoardItem): string {
  const shortId = item.id.split("-").at(-1) ?? item.id;
  return [
    padTaskCell(formatTaskTableStatus(item.status), TASK_TABLE_STATUS_WIDTH),
    padTaskCell(formatTaskPriority(item.priority), TASK_TABLE_PRIORITY_WIDTH),
    padTaskCell(item.title, TASK_TABLE_TITLE_WIDTH),
    padTaskCell(shortId, TASK_TABLE_ID_WIDTH),
    padTaskCell(String(item.evidenceCount), TASK_TABLE_EVIDENCE_WIDTH, "end"),
    padTaskCell(String(item.blockedByCount), TASK_TABLE_BLOCKER_WIDTH, "end"),
  ].join(" ");
}

function padTaskCell(text: string, width: number, justify: "start" | "end" = "start"): string {
  const clipped = truncate(text, width);
  return justify === "end" ? clipped.padStart(width) : clipped.padEnd(width);
}

function formatTaskTableStatus(status: TaskBoardItem["status"]): string {
  switch (status) {
    case "pending":
      return "RDY";
    case "in_progress":
      return "RUN";
    case "completed":
      return "DONE";
  }
}

function formatTaskPriority(priority: TaskBoardItem["priority"]): string {
  const text = String(priority);
  return text.length === 0 ? "-" : text.slice(0, 1).toUpperCase();
}

function wrapTaskDetailText(text: string): readonly string[] {
  const width = 72;
  const words = text.split(/\s+/).filter(Boolean);
  const lines: string[] = [];
  let line = "";
  for (const word of words) {
    const candidate = line.length === 0 ? word : `${line} ${word}`;
    if (candidate.length <= width) {
      line = candidate;
      continue;
    }
    if (line.length > 0) lines.push(line);
    line = word.length > width ? truncate(word, width) : word;
  }
  if (line.length > 0) lines.push(line);
  return lines.length > 0 ? lines : [text];
}

function buildTimelineModal(
  state: TimelineModalState,
  returnTarget: "command-palette" | undefined,
): ModalOptions {
  const milestones = state.snapshot.timelineMilestones ?? [];
  const selected = milestones[state.modal.selectedIndex];

  return {
    mode: "split",
    title: "Mission Timeline",
    eyebrow: "Milestone progress and feature assignments.",
    items: milestones.length > 0
      ? milestones.map((m) => {
          const bar = buildProgressBar(m.progressPct, 10);
          return {
            label: m.title,
            detail: `${m.kind} -- ${m.profile}`,
            hint: `${bar} ${m.progressPct}%`,
          };
        })
      : [{ label: "No milestones", selectable: false, tone: "muted" as const }],
    selectedIndex: Math.min(state.modal.selectedIndex, Math.max(0, milestones.length - 1)),
    detailItems: selected
      ? [
          { text: `Milestone: ${selected.title}`, section: "Overview" },
          { text: `Order: ${selected.order} -- ${selected.kind} (${selected.profile})` },
          { text: `Progress: ${selected.progressPct}%` },
          { text: "", section: "Features" },
          ...selected.features.map((f) => ({
            text: `${FEATURE_STATUS_LABEL[f.status] ?? f.status} ${f.title}`,
            detail: f.id,
          })),
        ]
      : [{ text: "Select a milestone to view features" }],
    footer: buildListOverlayFooter(returnTarget),
    returnTarget,
    renderSpec: buildOverlayRenderSpec("timeline"),
  };
}

function buildPrincipleReviewModal(
  state: PrincipleReviewModalState,
  returnTarget: "command-palette" | undefined,
): ModalOptions {
  const rows = state.snapshot.principleEffectiveness ?? [];
  const selected = rows[state.modal.selectedIndex];
  const eyebrow = rows.length === 0
    ? "No principle outcomes recorded yet. Launch work and ingest replies to score principles."
    : "Sorted worst-first. [GATE] = gating principle, [adv] = advisory. Pending outcomes wait for reply.";

  return {
    mode: "split",
    title: "Principle Effectiveness",
    eyebrow,
    items: rows.length > 0
      ? rows.map((row) => {
          const badge = row.mode === "gate" ? "[GATE]" : "[adv] ";
          const effStr = row.effectivenessPct === undefined
            ? " -- "
            : `${row.effectivenessPct}%`.padStart(4);
          const sampleNote = row.lowSample ? " (low sample)" : "";
          return {
            label: `${badge} ${row.name}`,
            detail: `eff ${effStr} -- ${row.helpful}/${row.helpful + row.unhelpful} decided${sampleNote}`,
            hint: row.pending > 0 ? `+${row.pending} pending` : "",
            tone: row.lowSample ? "muted" as const : undefined,
          };
        })
      : [{ label: "No principles with outcomes yet", selectable: false, tone: "muted" as const }],
    selectedIndex: Math.min(state.modal.selectedIndex, Math.max(0, rows.length - 1)),
    detailItems: selected
      ? [
          { text: selected.name, section: "Principle" },
          { text: `ID: ${selected.id}`, tone: "muted" as const },
          { text: `Mode: ${selected.mode}` },
          {
            text: selected.effectivenessPct === undefined
              ? "Effectiveness: no decided outcomes yet"
              : `Effectiveness: ${selected.effectivenessPct}% (${selected.helpful} helpful / ${selected.helpful + selected.unhelpful} decided)`,
          },
          ...(selected.pending > 0
            ? [{ text: `Pending: ${selected.pending} launch outcome(s) awaiting a reply`, tone: "muted" as const }]
            : []),
          ...(selected.lowSample
            ? [{
                text: "Sample size is below the 3-outcome threshold; treat the ratio as weak signal.",
                tone: "muted" as const,
              }]
            : []),
          ...(selected.recentKickbackExamples.length > 0
            ? [
                { text: "", section: "Recent kickbacks" },
                ...selected.recentKickbackExamples.map((example) => ({
                  text: example,
                  tone: "muted" as const,
                })),
              ]
            : []),
        ]
      : [{ text: "Select a principle to view its outcomes" }],
    footer: buildListOverlayFooter(returnTarget),
    returnTarget,
    renderSpec: buildOverlayRenderSpec("principle-review"),
  };
}

function buildHelpModal(
  state: AppState,
  returnTarget: "command-palette" | undefined,
): ModalOptions {
  const specs = getMissionControlCommandSpecs(state.snapshot.mode);
  return {
    mode: "info",
    title: "Keyboard Shortcuts",
    items: [
      ...specs.map((spec) => ({
        text: `${spec.key.padEnd(8)} ${spec.label}`,
        detail: spec.detail,
        section: spec.section,
      })),
      { text: "", section: "Navigation" },
      { text: "j/k      Up/Down" },
      { text: "Enter    Select / Drill in" },
      { text: "Esc      Back / Close" },
      { text: "Ctrl+P   Command Palette" },
      { text: "/        Quick search" },
      { text: "Ctrl+Y   Copy mode" },
    ],
    footer: "Press any key to dismiss",
    returnTarget,
    renderSpec: buildOverlayRenderSpec("help"),
  };
}

function buildAutopilotModal(
  state: AutopilotModalState,
  returnTarget: "command-palette" | undefined,
): ModalOptions {
  const tasks = state.snapshot.autopilot?.tasks ?? [];
  const selected = tasks[state.modal.selectedIndex];

  return {
    mode: "split",
    title: "Autopilot",
    eyebrow: "Per-task verdict, retry, and wall-clock status.",
    items: tasks.length > 0
      ? tasks.map((task) => ({
          label: task.taskId,
          detail: task.latestVerdict?.decision ?? "no verdict",
          hint: task.intent.length > 20 ? `${task.intent.slice(0, 20)}…` : task.intent,
        }))
      : [{ label: "No tasks for this mission", selectable: false, tone: "muted" as const }],
    selectedIndex: Math.min(state.modal.selectedIndex, Math.max(0, tasks.length - 1)),
    detailItems: selected
      ? [
          { text: selected.intent, section: "Intent" },
          { text: `Task: ${selected.taskId}`, tone: "muted" as const },
          { text: "", section: "Verdict" },
          { text: selected.latestVerdict
              ? `${selected.latestVerdict.decision}  at ${selected.latestVerdict.at}`
              : "(none yet)" },
          { text: "", section: "Budget" },
          { text: selected.maxRetries !== undefined
              ? `Retries: ${selected.retryCount}/${selected.maxRetries}`
              : `Retries: ${selected.retryCount}` },
          { text: selected.maxWallClockSeconds !== undefined
              ? `Wall-clock: ${selected.wallClockElapsedSeconds}s/${selected.maxWallClockSeconds}s`
              : `Wall-clock: ${selected.wallClockElapsedSeconds}s` },
          ...(selected.lastUpdatedAt ? [{ text: `Updated: ${selected.lastUpdatedAt}`, tone: "muted" as const }] : []),
        ]
      : [{ text: "Select a task to view autopilot details" }],
    footer: buildListOverlayFooter(returnTarget),
    returnTarget,
    renderSpec: buildOverlayRenderSpec("autopilot"),
  };
}

function buildProgressBar(pct: number, width: number): string {
  const filled = Math.round((pct / 100) * width);
  return "[" + "#".repeat(filled) + "-".repeat(width - filled) + "]";
}

function buildTabbedOverlayFooter(returnTarget: "command-palette" | undefined): string {
  return returnTarget === "command-palette"
    ? "Tab cycle tabs · Left back · Esc close"
    : "Tab cycle tabs · Esc close";
}

function buildListOverlayFooter(returnTarget: "command-palette" | undefined): string {
  return returnTarget === "command-palette"
    ? "Use arrows · Left back · Esc close"
    : "Use arrows · Esc close";
}

function buildConfigRowBlock(
  rows: readonly MissionControlConfigRow[],
  keys: readonly string[],
) {
  return keys.flatMap((keyPath) => {
    const row = rows.find((candidate) => candidate.keyPath === keyPath);
    if (!row) return [];
    const valueText = row.displayValueText || row.valueText;
    const optionText = row.options && row.options.length > 0 ? `  [ ${row.options.join(" | ")} ]` : "";
    return [{ text: `${formatConfigLine(row.label, valueText)}${optionText}` }];
  });
}

function formatConfigLine(label: string, value: string): string {
  return `${label.padEnd(20, " ")} ${value}`;
}

function findConfigValue(rows: readonly MissionControlConfigRow[], keyPath: string): string | undefined {
  return rows.find((row) => row.keyPath === keyPath)?.displayValueText;
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
  const modal = state.modal;
  if (modal.kind !== "config") return "";
  const tabs = state.snapshot.configInspector?.tabs ?? [];
  const labelText = tabs
    .map((tab) => {
      const label = getConfigTabDisplayLabel(tab);
      return tab === modal.tab ? `[${label}]` : label;
    })
    .join(" ");
  return labelText;
}

function buildConfigTitle(state: AppState): string {
  if (state.modal.kind !== "config") return "Config";
  const selectedRow = getConfigRowsForTab(
    state.snapshot.configInspector ?? null,
    state.modal.tab,
    state.modal.findQuery,
  )[state.modal.selectedRowIndex];
  switch (state.modal.phase) {
    case "edit-inline":
      return selectedRow?.label ?? "Edit Setting";
    case "choose-scope":
      return "Choose Save Scope";
    case "confirm-write":
      return selectedRow?.label ?? "Review Change";
    case "write-result":
      return "Change Saved";
    case "browse":
    default:
      return state.modal.tab === "doctor" ? "Problems" : "Config Palette";
  }
}

function buildConfigEyebrow(state: AppState): string | undefined {
  if (state.modal.kind !== "config") return undefined;
  const rows = getConfigRowsForTab(
    state.snapshot.configInspector ?? null,
    state.modal.tab,
    state.modal.findQuery,
  );
  const selectedRow = rows[state.modal.selectedRowIndex];
  switch (state.modal.phase) {
    case "edit-inline":
      if (!selectedRow) return "Choose a value, adjust scope if needed, and preview before saving.";
      return selectedRow.summary;
    case "choose-scope":
      return selectedRow?.label;
    case "confirm-write":
      return "Review the pending change before applying it.";
    case "write-result":
      return selectedRow?.label;
    case "browse":
    default:
      if (state.modal.tab === "doctor") {
        return "Fix anything here before you trust the next run.";
      }
      return `${buildConfigTabs(state)}\n${buildConfigActionRow(state)}`;
  }
}

function buildConfigListTitle(state: AppState): string {
  if (state.modal.kind !== "config") return "List";
  switch (state.modal.phase) {
    case "edit-inline":
      return "Choices";
    case "confirm-write":
      return "Summary";
    case "choose-scope":
      return "Scope";
    case "browse":
    default:
      return "Results";
  }
}

function buildConfigDetailTitle(state: AppState): string {
  if (state.modal.kind !== "config") return "Detail";
  switch (state.modal.phase) {
    case "edit-inline":
      return "Editor";
    case "confirm-write":
      return "Preview";
    case "choose-scope":
      return "Guidance";
    case "browse":
    default:
      return "Details";
  }
}

function buildConfigDetailItems(
  state: ConfigModalState,
  row: MissionControlConfigRow | undefined,
) {
  if (!row) {
    return [{
      text: state.modal.findQuery ? "Try a different search term." : "Choose a setting on the left to inspect it.",
      section: "Config",
      tone: "muted" as const,
    }];
  }

  switch (state.modal.phase) {
    case "choose-scope":
      return buildConfigScopeDetailItems(state, row);
    case "edit-inline":
      return buildConfigEditDetailItems(state, row);
      case "confirm-write":
        return buildConfigConfirmDetailItems(state, row);
      case "browse":
      default:
        return state.modal.tab === "doctor"
          ? buildConfigProblemsDetailItems(row)
          : buildConfigBrowseDetailItems(state, row);
    }
    }

function formatOptionLabel(_row: MissionControlConfigRow, option: string): string {
  return option;
}

function displayDraftValue(row: MissionControlConfigRow, draftValue?: string): string {
    return formatOptionLabel(row, draftValue ?? row.effectiveValueText);
  }

function buildConfigItems(
  state: ConfigModalState,
  rows: readonly MissionControlConfigRow[],
  selectedRow: MissionControlConfigRow | undefined,
): {
  items: readonly {
    label: string;
    detail?: string;
    hint?: string;
    section?: string;
    selectable?: boolean;
    tone?: "default" | "muted" | "accent";
  }[];
  selectedIndex: number;
} {
  if (state.modal.phase === "choose-scope") {
      if (selectedRow && isGlobalOnlyConfigKey(selectedRow.keyPath)) {
        return {
          items: [
            {
              label: "Global config",
              section: "Save destination",
            },
            {
              label: "This setting is global-only",
              selectable: false,
              tone: "muted",
            },
            {
              label: "Project config values are ignored",
              selectable: false,
              tone: "muted",
            },
          ],
          selectedIndex: 0,
        };
      }
      return {
      items: [
        {
          label: "Project config",
          section: "Choose where to save this",
        },
        {
          label: "Only this project changes",
          selectable: false,
          tone: "muted",
        },
        {
          label: "Global config",
        },
        {
          label: "All projects use this value",
          selectable: false,
          tone: "muted",
        },
        {
          label: state.modal.selectedScope === "project" ? "Project config" : "Global config",
          section: "Current target",
          selectable: false,
          tone: "accent",
        },
        {
          label: "Project config",
          section: "Recommended now",
          selectable: false,
        },
      ],
      selectedIndex: state.modal.selectedScope === "project" ? 0 : 1,
    };
  }
  if (state.modal.phase === "edit-inline" && selectedRow?.options?.length) {
      return {
        items: buildValueChoiceItems(selectedRow),
        selectedIndex: Math.max(0, selectedRow.options.indexOf(state.modal.draftValue ?? selectedRow.effectiveValueText)),
      };
    }
  if (state.modal.phase === "confirm-write") {
    return {
      items: [
        { label: "Setting", detail: selectedRow?.label ?? "Unknown", section: "Change summary", selectable: false },
        { label: "Old value", detail: selectedRow?.effectiveDisplayValueText ?? "unset", selectable: false },
        { label: "New value", detail: selectedRow ? displayDraftValue(selectedRow, state.modal.draftValue) : "unset", selectable: false },
        { label: "Save to", detail: state.modal.selectedScope === "project" ? "project config" : "global config", selectable: false },
      ],
      selectedIndex: 0,
    };
  }
  if (rows.length === 0) {
    return {
      items: [{
        label: state.modal.findQuery ? "No settings match your search" : "No settings available",
        selectable: false,
        tone: "muted",
      }],
      selectedIndex: 0,
    };
  }
    return {
      items: rows.map((row, index) => ({
        label: row.label,
        detail: row.displayValueText,
        section: index === 0 || rows[index - 1]?.section !== row.section ? row.section : undefined,
      })),
      selectedIndex: Math.min(state.modal.selectedRowIndex, Math.max(0, rows.length - 1)),
    };
  }

function buildConfigBrowseDetailItems(
  state: ConfigModalState,
  row: MissionControlConfigRow,
) {
    const targetScope = resolveConfigScopeForKey(row.keyPath, state.modal.selectedScope);
    const globalOnly = isGlobalOnlyConfigKey(row.keyPath);
    return [
      { text: row.label, tone: "accent" as const, style: "block" as const },
      { text: row.summary },
      { text: row.effectiveDisplayValueText, section: "Using now", tone: "accent" as const, style: "block" as const },
      ...buildSavedValueItems(row, "Saved values"),
      { text: targetScope === "project" ? "Project config" : "Global config", section: "Next save target", tone: "accent" as const },
      ...(!globalOnly ? [{ text: "Press S to switch between project and global." }] : []),
      ...(globalOnly ? [{ text: "Project overrides are ignored for this setting.", section: "Scope rule" }] : []),
      { text: row.impactText, section: "Why it matters" },
      { text: globalOnly ? "Enter edit   P preview" : "Enter edit   S scope   P preview", section: "Next actions" },
    ];
  }

function buildConfigEditDetailItems(
  state: ConfigModalState,
  row: MissionControlConfigRow,
) {
    const targetScope = resolveConfigScopeForKey(row.keyPath, state.modal.selectedScope);
    const globalOnly = isGlobalOnlyConfigKey(row.keyPath);
    return [
      { text: row.label, tone: "accent" as const, style: "block" as const },
      { text: row.summary },
      { text: displayDraftValue(row, state.modal.draftValue), section: "Selected value", tone: "accent" as const, style: "block" as const },
      { text: row.effectiveDisplayValueText, section: "Using now", tone: "accent" as const },
      ...buildSavedValueItems(row, "Saved values"),
      { text: targetScope === "project" ? "Project config" : "Global config", section: "Next save target", tone: "accent" as const },
      ...(!globalOnly ? [{ text: "Press S to switch between project and global." }] : []),
      ...(globalOnly ? [{ text: "Project overrides are ignored for this setting.", section: "Scope rule" }] : []),
      { text: row.impactText, section: "Why it matters" },
      { text: globalOnly ? "Up/Down choose   P preview   Enter review" : "Up/Down choose   S scope   P preview   Enter review", section: "Next actions" },
    ];
  }

function buildConfigScopeDetailItems(
  state: ConfigModalState,
  row: MissionControlConfigRow,
) {
    if (isGlobalOnlyConfigKey(row.keyPath)) {
      return [
        { text: "Global-only setting", tone: "accent" as const, style: "block" as const },
        { text: "Mission Control background mode is always read from global config." },
        { text: "Project config values are ignored.", section: "What this means" },
        { text: "Global config", section: "Current target", tone: "accent" as const },
      ];
    }
    const projectSelected = state.modal.selectedScope === "project";
  return [
    { text: "What each option means", tone: "accent" as const, style: "block" as const },
    { text: "Project config", section: "What each option means" },
    { text: "Only this repo uses the new value." },
    { text: "Global config", section: "What each option means" },
    { text: "Other projects use this value unless they override it locally." },
    { text: projectSelected ? "Project config" : "Global config", section: "Current target", tone: "accent" as const },
    { text: "Project config", section: "Recommended now" },
  ];
}

function buildConfigConfirmDetailItems(
  state: ConfigModalState,
  row: MissionControlConfigRow,
) {
    const targetScope = resolveConfigScopeForKey(row.keyPath, state.modal.selectedScope);
  const previewLines = state.modal.kind === "config" && state.modal.preview
    ? state.modal.preview.content.split("\n").filter((line) => line.length > 0).slice(0, 8)
    : [];
  return [
    { text: row.label, section: "Pending change", tone: "accent" as const, style: "block" as const },
    { text: `${row.effectiveDisplayValueText} -> ${displayDraftValue(row, state.modal.draftValue)}`, section: "Pending change" },
    { text: `Save scope: ${targetScope === "project" ? "project config" : "global config"}`, section: "Pending change" },
    { text: state.modal.kind === "config" && state.modal.preview ? state.modal.preview.path : "Preview unavailable", section: "Preview file", tone: "accent" as const, style: "block" as const },
    ...previewLines.map((line, index) => ({ text: line, section: index === 0 ? "Preview file" : undefined })),
    { text: "Enter apply   Esc back", section: "Next actions" },
  ];
}

  function buildConfigResultItems(
    state: AppState,
    row: MissionControlConfigRow | undefined,
  ) {
    const scope = row && state.modal.kind === "config"
      ? resolveConfigScopeForKey(row.keyPath, state.modal.selectedScope)
      : state.modal.kind === "config"
        ? state.modal.selectedScope
        : "project";
    const scopeLabel = scope === "global" ? "global config" : "project config";
  return [
    {
      text: `Saved to ${scopeLabel}.`,
      section: row?.label ?? "Setting",
      tone: "accent" as const,
    },
    { text: "Maestro reloaded the latest config." },
    ...(row ? [{ text: row.effectiveDisplayValueText, section: "Using now", tone: "accent" as const, style: "block" as const }] : []),
    ...(row ? [{ text: row.impactText, section: "What happens next" }] : []),
    { text: "Enter continue", section: "Next actions" },
  ];
}

function buildConfigProblemsDetailItems(row: MissionControlConfigRow) {
  return [
    { text: row.label, tone: "accent" as const, style: "block" as const },
    { text: row.displayValueText },
    { text: row.summary, section: "How to fix it" },
    { text: row.impactText, section: "Why it matters" },
    { text: "Enter inspect   R recheck", section: "Next actions" },
  ];
}

function buildSavedValueItems(row: MissionControlConfigRow, section = "Also set in") {
  const items = [
    row.projectDisplayValueText !== undefined ? { label: "Project", value: row.projectDisplayValueText } : undefined,
    row.globalDisplayValueText !== undefined ? { label: "Global", value: row.globalDisplayValueText } : undefined,
    row.defaultDisplayValueText !== undefined ? { label: "Default", value: row.defaultDisplayValueText } : undefined,
  ].filter((item): item is { label: string; value: string } => Boolean(item));
  const meaningful = items.filter((item) => item.value !== "unset");
  if (meaningful.length === 0) {
    return [{ text: "No saved overrides.", section, tone: "muted" as const }];
  }
  return meaningful.map((item, index) => ({
    text: `${item.label}   ${item.value}`,
    section: index === 0 ? section : undefined,
    tone: "muted" as const,
  }));
}

function buildValueChoiceItems(
  row: MissionControlConfigRow,
) {
  return [
    ...row.options!.map((option, index) => ({
      label: formatOptionLabel(row, option),
      detail: option === row.effectiveValueText ? "using now" : undefined,
      section: index === 0 ? "Choose a value" : undefined,
    })),
  ];
}

function buildConfigActionRow(state: AppState): string {
  if (state.modal.kind !== "config") return "";
  const searchText = state.modal.findQuery !== undefined
    ? state.modal.findQuery.length > 0
      ? state.modal.findQuery
      : "type to search"
    : "search";
  switch (state.modal.phase) {
    case "edit-inline":
      return "Up/Down choose    S scope    P preview    Enter review";
    case "confirm-write":
      return "Enter apply    Esc back";
    case "choose-scope":
      return "Up/Down scope    Enter use scope";
    case "browse":
    default:
      return `/ ${searchText}    Enter edit    S scope    P preview    R reload`;
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

function formatTaskStatus(status: keyof typeof FEATURE_TASK_STATUS_LABEL): string {
  return FEATURE_TASK_STATUS_LABEL[status].toLowerCase();
}

export function isSelectableListModal(kind: AppState["modal"]["kind"]): kind is "feature-browser" | "dependencies" | "memory" | "graph" | "agent-grid" | "dispatch" | "event-stream" | "task-board" | "timeline" {
  return kind === "feature-browser"
    || kind === "dependencies"
    || kind === "memory"
    || kind === "graph"
    || kind === "agent-grid"
    || kind === "dispatch"
    || kind === "event-stream"
    || kind === "task-board"
    || kind === "timeline";
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
    case "config":
      return { type: "open-config" };
    case "memory":
      return { type: "open-memory" };
    case "graph":
      return { type: "open-graph" };
    case "agent-grid":
      return { type: "open-agent-grid" };
    case "dispatch":
      return { type: "open-dispatch" };
    case "event-stream":
      return { type: "open-event-stream" };
    case "task-board":
      return { type: "open-task-board" };
    case "timeline":
      return { type: "open-timeline" };
    case "principle-review":
      return { type: "open-principle-review" };
    case "help":
      return { type: "open-help" };
    case "exit":
      return { type: "quit" };
  }
}
