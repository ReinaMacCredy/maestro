/**
 * Modal option builders and command palette glue.
 * Extracted from index.ts -- builds ModalOptions from AppState.
 */
import { homedir } from "node:os";
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
import { getValidFeatureTransitions } from "@/features/mission";
import { TASK_STATUSES } from "@/features/task";
import { FEATURE_STATUS_LABEL, FEATURE_TASK_STATUS_LABEL, TASK_STATUS_COLUMN_LABEL, AGENT_STATUS_LABEL } from "../theme.js";
import { shortenSessionId } from "../session-id.js";
import { GRAPH_DIR } from "@/shared/domain/defaults.js";
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
          returnTarget,
          renderSpec: buildOverlayRenderSpec("handoffs"),
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

  return undefined;
}

function buildMemoryModal(
  state: MemoryModalState,
  returnTarget: "command-palette" | undefined,
): ModalOptions {
  const memory = state.snapshot.memory;
  const eyebrow = `${buildMemoryTabs(state)}\n${buildMemoryHelpText()}`;

  if (state.modal.tab === "overview") {
    return {
      mode: "info",
      title: "Memory",
      eyebrow,
      items: buildMemoryOverviewItems(memory),
      footer: buildTabbedOverlayFooter(returnTarget),
      returnTarget,
      renderSpec: buildOverlayRenderSpec("memory"),
    };
  }

  if (state.modal.tab === "corrections") {
    const corrections = memory?.corrections ?? [];
    const selectedCorrection = corrections[state.modal.selectedItemIndex];
    return {
      mode: "split",
      title: "Memory",
      eyebrow,
      items: corrections.length > 0
        ? corrections.map((correction) => ({
            label: `${correction.severity === "hard" ? "[!]" : "[ ]"} ${correction.rule}`,
            detail: correction.trigger.keywords.join(", ") || "no keywords",
          }))
        : [{ label: "No saved corrections", selectable: false, tone: "muted" }],
      selectedIndex: Math.min(state.modal.selectedItemIndex, Math.max(0, corrections.length - 1)),
      detailItems: buildCorrectionDetailItems(selectedCorrection),
      footer: buildTabbedOverlayFooter(returnTarget),
      returnTarget,
      renderSpec: buildOverlayRenderSpec("memory"),
    };
  }

  if (state.modal.tab === "learnings") {
    return {
      mode: "info",
      title: "Memory",
      eyebrow,
      items: buildLearningActivityItems(memory, state.snapshot.configInspector ?? null),
      footer: buildTabbedOverlayFooter(returnTarget),
      returnTarget,
      renderSpec: buildOverlayRenderSpec("memory"),
    };
  }

  if (state.modal.tab === "config") {
    return {
      mode: "info",
      title: "Memory System",
      eyebrow,
      items: buildMemoryConfigItems(state.snapshot.configInspector ?? null),
      footer: buildTabbedOverlayFooter(returnTarget),
      returnTarget,
      renderSpec: buildOverlayRenderSpec("config"),
    };
  }

  const assertions = memory?.ratchetSuite.assertions ?? [];
  return {
    mode: "split",
    title: "Memory",
    eyebrow,
    items: assertions.length > 0
      ? assertions.map((assertion) => ({
          label: assertion.rule,
          detail: assertion.check,
        }))
      : [{ label: "No ratchet assertions", selectable: false, tone: "muted" }],
    selectedIndex: Math.min(state.modal.selectedItemIndex, Math.max(0, assertions.length - 1)),
    detailItems: buildRatchetDetailItems(memory, assertions[state.modal.selectedItemIndex]),
    footer: buildTabbedOverlayFooter(returnTarget),
    returnTarget,
    renderSpec: buildOverlayRenderSpec("memory"),
  };
}

function buildGraphModal(
  state: GraphModalState,
  returnTarget: "command-palette" | undefined,
): ModalOptions {
  const graphContext = state.snapshot.memory?.graphContext;
  const entries = buildGraphListEntries(graphContext);
  const selectedEntry = entries[state.modal.selectedItemIndex];
  return {
    mode: "split",
    title: "Project Graph",
    eyebrow: "Cross-project relationships and impact analysis.",
    items: entries.length > 0
      ? entries.map((entry) => ({
          label: entry.label,
          detail: entry.detail,
        }))
      : [{ label: "No related projects", selectable: false, tone: "muted" }],
    selectedIndex: Math.min(state.modal.selectedItemIndex, Math.max(0, entries.length - 1)),
    detailItems: buildGraphDetailItems(graphContext, selectedEntry),
    footer: buildListOverlayFooter(returnTarget),
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

function buildAgentGridModal(
  state: AgentGridModalState,
  returnTarget: "command-palette" | undefined,
): ModalOptions {
  const grid = state.snapshot.agentGrid ?? [];
  const selected = grid[state.modal.selectedIndex];
  return {
    mode: "split",
    title: "Agent Grid",
    eyebrow: "Worker status and feature assignments.",
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
          { text: `Worker: ${formatAgentLabel(selected.agentType)}` },
          { text: `Status: ${AGENT_STATUS_LABEL[selected.status]}` },
          ...(selected.activeFeatureId
            ? [{ text: `Active: ${selected.activeFeatureId}`, detail: selected.activeFeatureTitle }]
            : []),
          { text: `Progress: ${selected.completedCount}/${selected.featureCount} features done` },
          ...(selected.pendingHandoffCount > 0
            ? [{ text: `Pending handoffs: ${selected.pendingHandoffCount}` }]
            : []),
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
  if (phase === "generating") footer = "Generating worker prompt...";
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
    items: queue.length > 0
      ? queue.map((item) => ({
          label: item.featureTitle,
          detail: `${item.agentType} -- ${item.milestoneTitle}`,
          hint: item.featureId,
        }))
      : [{ label: "No features ready for dispatch", selectable: false, tone: "muted" as const }],
    selectedIndex: Math.min(state.modal.selectedIndex, Math.max(0, queue.length - 1)),
    detailItems: selected
      ? [
          { text: `Feature: ${selected.featureId}`, detail: selected.featureTitle },
          { text: `Milestone: ${selected.milestoneTitle} (#${selected.milestoneOrder})` },
          { text: `Worker: ${formatAgentLabel(selected.agentType)}` },
        ]
      : [{ text: "Select a feature to view dispatch details" }],
    footer,
    returnTarget,
    renderSpec: buildOverlayRenderSpec("dispatch"),
  };
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
    items: items.length > 0
      ? items.map((item) => ({
          label: item.title,
          detail: item.assignee ? `@${item.assignee}` : undefined,
          hint: item.id,
        }))
      : [{ label: `No ${TASK_STATUS_COLUMN_LABEL[col].toLowerCase()} tasks`, selectable: false, tone: "muted" as const }],
    selectedIndex: Math.min(state.modal.selectedIndex, Math.max(0, items.length - 1)),
    detailItems: selected
      ? [
          { text: selected.title },
          { text: `ID: ${selected.id}` },
          { text: `Priority: ${selected.priority}` },
          ...(selected.assignee ? [{ text: `Assignee: ${selected.assignee}` }] : []),
          ...(selected.labels.length > 0 ? [{ text: `Labels: ${selected.labels.join(", ")}` }] : []),
          ...(selected.blockedByCount > 0 ? [{ text: `Blocked by: ${selected.blockedByCount}` }] : []),
        ]
      : [{ text: "Select a task to view details" }],
    footer: `Tab/[/] columns -- ${buildListOverlayFooter(returnTarget)}`,
    returnTarget,
    renderSpec: buildOverlayRenderSpec("task-board"),
  };
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
    ? "No principle outcomes recorded yet. Create a handoff that passes gates, then reply to see scores."
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
            ? [{ text: `Pending: ${selected.pending} handoff(s) awaiting a reply`, tone: "muted" as const }]
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

function buildProgressBar(pct: number, width: number): string {
  const filled = Math.round((pct / 100) * width);
  return "[" + "#".repeat(filled) + "-".repeat(width - filled) + "]";
}

function buildMemoryTabs(state: MemoryModalState): string {
  const tabs = ["overview", "corrections", "learnings", "ratchet", "config"] as const;
  return tabs.map((tab) => tab === state.modal.tab ? `[${tab}]` : tab).join(" ");
}

function buildMemoryHelpText(): string {
  return "Tab or [ ] switch tabs. Up and Down move through saved items.";
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

function buildMemoryOverviewItems(memory: MissionControlSnapshot["memory"]): readonly ModalInfoItem[] {
  if (!memory) {
    return [{ text: "No memory system data available", tone: "muted" as const }];
  }

  const correctionPreview = memory.corrections.slice(0, 4);
  const compiledAt = formatDateTimeCompact(memory.stats.learnings.compiledAt);
  const rawSinceCompile = memory.compiledLearnings
    ? Math.max(0, memory.rawLearnings.length - memory.compiledLearnings.rawCount)
    : memory.rawLearnings.length;
  const ratchetStatus = memory.stats.ratchet.lastResult?.toUpperCase() ?? "No run";

  return [
    {
      text: `Corrections: ${memory.stats.corrections.total}        Learnings: ${memory.stats.learnings.rawCount} raw / ${compiledAt ? `compiled ${compiledAt}` : "not compiled"}        Ratchet: ${memory.stats.ratchet.assertions}`,
      section: "Summary",
      tone: "accent" as const,
    },
    { text: "" },
    { text: "Corrections", section: "Corrections", tone: "accent" as const },
    ...(correctionPreview.length > 0
      ? correctionPreview.map((correction) => ({
          text: `${correction.severity === "hard" ? "[!]" : "[ ]"} ${correction.rule}    triggers: ${formatTriggerPreview(correction.trigger.keywords, correction.trigger.fileGlobs)}    severity: ${correction.severity.toUpperCase()}`,
        }))
      : [{ text: "No saved corrections", tone: "muted" as const }]),
    { text: `${memory.stats.corrections.total} total  [View All -->]` },
    { text: "" },
    { text: "Learnings (compiled)", section: "Learnings", tone: "accent" as const },
    { text: compiledAt ? `Last compiled: ${compiledAt}  (${rawSinceCompile} raw entries since)` : "Last compiled: not yet" },
    ...(memory.compiledLearnings
      ? splitIntoBullets(memory.compiledLearnings.summary)
      : [{ text: "- No compiled learning summary yet", tone: "muted" as const }]),
    { text: "[Full -->]" },
    ...(memory.stats.learnings.staleDays !== undefined
      ? [{ text: `Staleness: ${memory.stats.learnings.staleDays} day(s)` }]
      : []),
    { text: "" },
    { text: "Ratchet", section: "Ratchet", tone: "accent" as const },
    { text: `Suite: ${memory.stats.ratchet.assertions} assertions   Last run: ${ratchetStatus}` },
    { text: memory.ratchetBaseline ? `Baseline score: ${formatBaselineScore(memory.ratchetBaseline.passCount, memory.ratchetSuite.assertions.length)}` : "Baseline score: n/a" },
    ...memory.ratchetSuite.assertions.slice(0, 4).map((assertion) => ({
      text: `[ok] ${assertion.id}    promoted ${formatDateOnly(assertion.createdAt)}`,
    })),
    { text: "" },
      { text: "Project Graph", section: "Graph", tone: "accent" as const },
      { text: `${memory.stats.graph.projects} projects · ${memory.stats.graph.links} links` },
      ...buildGraphImpactItems(memory.graphContext),
    ];
  }

function buildCorrectionDetailItems(correction: NonNullable<MissionControlSnapshot["memory"]>["corrections"][number] | undefined) {
  if (!correction) {
    return [{ text: "Choose a correction to inspect it.", tone: "muted" as const }];
  }

  return [
    { text: correction.rule, section: "Rule", tone: "accent" as const, style: "block" as const },
    { text: correction.source, section: "Source" },
    { text: correction.trigger.keywords.join(", ") || "none", section: "Keywords" },
    { text: correction.trigger.fileGlobs.join(", ") || "none", section: "Globs" },
    { text: correction.severity.toUpperCase(), section: "Severity" },
    { text: correction.promotedToRatchet ? `Promoted ${correction.promotedToRatchet}` : "Not promoted", section: "Ratchet" },
    { text: correction.createdAt, section: "Created" },
    { text: correction.updatedAt, section: "Updated" },
  ];
}

function buildLearningDetailItems(
  memory: MissionControlSnapshot["memory"],
  selectedLearning: NonNullable<MissionControlSnapshot["memory"]>["rawLearnings"][number] | undefined,
): readonly ModalInfoItem[] {
  if (!memory) {
    return [{ text: "No learning data available.", tone: "muted" as const }];
  }

  return [
    ...(selectedLearning
      ? [
          { text: selectedLearning.sessionDate, section: "Selected entry", tone: "accent" as const, style: "block" as const },
          { text: selectedLearning.branch ?? "no branch" },
          { text: selectedLearning.content },
          { text: "" },
        ]
      : [{ text: "Choose a raw learning entry.", tone: "muted" as const }]),
    { text: "Compiled summary", section: "Compiled" },
    ...(memory.compiledLearnings
      ? [
          { text: `Compiled ${memory.compiledLearnings.compiledAt}` },
          { text: `${memory.compiledLearnings.rawCount} raw entries included` },
          { text: memory.compiledLearnings.summary },
        ]
      : [
          { text: "Not compiled yet", tone: "muted" as const },
          { text: `${memory.rawLearnings.length} raw entr${memory.rawLearnings.length === 1 ? "y" : "ies"} waiting` },
        ]),
  ];
}

function buildRatchetDetailItems(
  memory: MissionControlSnapshot["memory"],
  assertion: NonNullable<MissionControlSnapshot["memory"]>["ratchetSuite"]["assertions"][number] | undefined,
): readonly ModalInfoItem[] {
  if (!memory) {
    return [{ text: "No ratchet data available.", tone: "muted" as const }];
  }

  return [
    ...(assertion
      ? [
          { text: assertion.rule, section: "Assertion", tone: "accent" as const, style: "block" as const },
          { text: assertion.check, section: "Check" },
          { text: assertion.correctionId, section: "Correction" },
          { text: assertion.createdAt, section: "Created" },
          { text: "" },
        ]
      : [{ text: "Choose a ratchet assertion.", tone: "muted" as const }]),
    { text: "Baseline", section: "Baseline" },
      ...(memory.ratchetBaseline
        ? [
            { text: `Pass count: ${memory.ratchetBaseline.passCount}` },
            { text: `Last run: ${memory.ratchetBaseline.lastRunAt}` },
          ]
        : [{ text: "No baseline recorded", tone: "muted" as const }]),
    ];
  }

function buildLearningActivityItems(
  memory: MissionControlSnapshot["memory"],
  inspector: MissionControlConfigInspector | null | undefined,
): readonly ModalInfoItem[] {
  if (!memory) {
    return [{ text: "No learning data available.", tone: "muted" as const }];
  }

  const activity = buildLearningActivitySeries(memory);
  const compiledAt = formatDateTimeCompact(memory.compiledLearnings?.compiledAt ?? memory.stats.learnings.compiledAt);
  const rawSinceCompile = memory.compiledLearnings
    ? Math.max(0, memory.rawLearnings.length - memory.compiledLearnings.rawCount)
    : memory.rawLearnings.length;
  const compileThreshold = getMemoryCompileThreshold(inspector) ?? 5;
  const remainingToCompile = Math.max(0, compileThreshold - rawSinceCompile);

  return [
    { text: "Learning Activity", section: "Activity", tone: "accent" as const },
    ...activity,
    { text: "" },
    { text: `Corrections captured:  ${memory.stats.corrections.total} total,  ${countRecentEntries(memory.corrections.map((entry) => entry.createdAt))} this week` },
    { text: `Learnings logged:      ${memory.stats.learnings.rawCount} total, ${countRecentEntries(memory.rawLearnings.map((entry) => entry.sessionDate))} this week` },
    { text: `Ratchet promotions:   ${memory.stats.ratchet.assertions} total,  ${countRecentEntries(memory.ratchetSuite.assertions.map((entry) => entry.createdAt))} this week` },
    { text: "" },
    { text: `Compile status: ${compiledAt ? `compiled ${compiledAt}` : "not compiled"}${compiledAt ? `   Next: ~${remainingToCompile} more entr${remainingToCompile === 1 ? "y" : "ies"}` : ""}` },
    ...(memory.compiledLearnings
      ? splitIntoBullets(memory.compiledLearnings.summary)
      : [{ text: "- No compiled learning summary yet", tone: "muted" as const }]),
  ];
}

function buildMemoryConfigItems(
  inspector: MissionControlConfigInspector | null | undefined,
): readonly ModalInfoItem[] {
  const rows = getConfigRowsForTab(inspector ?? null, "memory");
  if (rows.length === 0) {
    return [{ text: "No memory config is available for this workspace.", tone: "muted" as const }];
  }

  const items = [
    { text: formatConfigLine("Memory System", findConfigValue(rows, "memory.enabled") ?? "unset"), section: "Memory System", tone: "accent" as const },
    { text: "" },
    { text: "Corrections", section: "Corrections", tone: "accent" as const },
    ...buildConfigRowBlock(rows, [
      "memory.corrections.enabled",
      "memory.corrections.matching",
      "memory.corrections.auto_capture",
      "memory.corrections.severity_default",
    ]),
    { text: "" },
    { text: "Learnings", section: "Learnings", tone: "accent" as const },
    ...buildConfigRowBlock(rows, [
      "memory.learnings.enabled",
      "memory.learnings.compile_threshold",
      "memory.learnings.max_age_days",
    ]),
    { text: "" },
    { text: "Ratchet", section: "Ratchet", tone: "accent" as const },
    ...buildConfigRowBlock(rows, [
      "memory.ratchet.enabled",
      "memory.ratchet.enforcement",
    ]),
    { text: "" },
    { text: "Project Graph", section: "Project Graph", tone: "accent" as const },
    ...buildConfigRowBlock(rows, ["memory.graph.enabled"]),
    { text: formatConfigLine("Global Path", shortenHomePath(GRAPH_DIR)) },
    { text: "" },
    { text: "Read-only summary. Open Config to edit memory settings.", tone: "muted" as const },
  ];

  return items;
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

function getMemoryCompileThreshold(
  inspector: MissionControlConfigInspector | null | undefined,
): number | undefined {
  const rows = getConfigRowsForTab(inspector ?? null, "memory");
  const threshold = rows.find((row) => row.keyPath === "memory.learnings.compile_threshold");
  const rawValue = threshold?.effectiveValueText ?? threshold?.valueText;
  if (!rawValue) return undefined;
  const parsed = Number.parseInt(rawValue, 10);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function formatTriggerPreview(keywords: readonly string[], globs: readonly string[]): string {
  const parts = [...keywords, ...globs];
  if (parts.length === 0) return "none";
  return parts.slice(0, 2).join(", ");
}

function formatDateTimeCompact(value: string | undefined): string | undefined {
  if (!value) return undefined;
  return value.slice(0, 16).replace("T", " ");
}

function formatDateOnly(value: string): string {
  return value.slice(0, 10);
}

function formatBaselineScore(passCount: number, totalCount: number): string {
  if (totalCount === 0) return "0.00";
  return (passCount / totalCount).toFixed(2);
}

function splitIntoBullets(summary: string) {
  return summary
    .split(/\n+/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .map((line) => ({
      text: line.startsWith("-") ? line : `- ${line}`,
    }));
}

function shortenHomePath(path: string): string {
  const home = homedir();
  return path.startsWith(home) ? `~${path.slice(home.length)}` : path;
}

interface GraphListEntry {
  readonly label: string;
  readonly detail: string;
  readonly relationship?: NonNullable<NonNullable<MissionControlSnapshot["memory"]>["graphContext"]>["relationships"][number];
}

function buildGraphListEntries(
  graphContext: NonNullable<MissionControlSnapshot["memory"]>["graphContext"] | undefined,
): readonly GraphListEntry[] {
  if (!graphContext) return [];

  const currentProject = graphContext.currentProject;
  const entries: GraphListEntry[] = [];
  if (currentProject) {
    entries.push({
      label: `${currentProject.name}${currentProject.role ? ` (${currentProject.role})` : ""}`,
      detail: "Current project",
    });
  }

  for (const relationship of graphContext.relationships) {
    entries.push({
      label: `${relationship.project.name}${relationship.project.role ? ` (${relationship.project.role})` : ""}`,
      detail: `${relationship.direction} · ${relationship.edge.relation}${relationship.edge.detail ? ` · ${relationship.edge.detail}` : ""}`,
      relationship,
    });
  }

  return entries;
}

function buildGraphDetailItems(
  graphContext: NonNullable<MissionControlSnapshot["memory"]>["graphContext"] | undefined,
  entry: GraphListEntry | undefined,
) {
  if (!graphContext) {
    return [{ text: "No graph data available.", tone: "muted" as const }];
  }

  const currentProjectLabel = graphContext.currentProject
    ? `${graphContext.currentProject.name}${graphContext.currentProject.role ? ` (${graphContext.currentProject.role})` : ""}`
    : "Current project";
  const relationship = entry?.relationship;

  return [
    { text: `Current project: ${currentProjectLabel}`, section: "Current Project", tone: "accent" as const, style: "block" as const },
    { text: `Path: ${graphContext.currentProject?.path ?? "unknown path"}` },
    ...(relationship
      ? [
          { text: "" },
          { text: `Selected project: ${relationship.project.name}${relationship.project.role ? ` (${relationship.project.role})` : ""}`, section: "Selected Project" },
          { text: `Path: ${relationship.project.path}` },
          { text: `Relationship: ${relationship.direction} · ${relationship.edge.relation}` },
          ...(relationship.edge.detail ? [{ text: `Detail: ${relationship.edge.detail}` }] : []),
        ]
      : graphContext.relationships.length > 0
        ? [
            { text: "" },
            { text: "Relationships:", section: "Relationships" },
            ...graphContext.relationships.map((candidate) => ({
              text: `${candidate.direction === "outgoing" ? "-->" : "<--"} ${candidate.project.name} · ${candidate.edge.relation}${candidate.edge.detail ? ` · ${candidate.edge.detail}` : ""}`,
            })),
          ]
        : [{ text: "Choose a related project.", tone: "muted" as const }]),
    { text: "" },
    ...buildGraphImpactItems(graphContext),
    { text: `${graphContext.totalProjects} project(s) · ${graphContext.totalEdges} link(s)`, section: "Summary" },
  ];
}

function buildGraphImpactItems(
  graphContext: NonNullable<MissionControlSnapshot["memory"]>["graphContext"] | undefined,
) {
  if (!graphContext || graphContext.relationships.length === 0) {
    return [{ text: "[ok] No downstream graph impacts recorded", tone: "muted" as const }];
  }

  return graphContext.relationships
    .filter((relationship) => relationship.edge.detail)
    .map((relationship) => ({
      text: `[!] Changing ${relationship.edge.detail} will impact: ${relationship.project.name}`,
    }));
}

function buildLearningActivitySeries(
  memory: NonNullable<MissionControlSnapshot["memory"]>,
) {
  const dayKeys = buildRecentDayKeys(memory);
  const labelLine = dayKeys.map((dayKey) => formatShortDay(dayKey).padEnd(9, " ")).join("");
  const correctionCounts = countEntriesByDay(memory.corrections.map((entry) => entry.createdAt), dayKeys);
  const learningCounts = countEntriesByDay(memory.rawLearnings.map((entry) => entry.sessionDate), dayKeys);
  const ratchetCounts = countEntriesByDay(memory.ratchetSuite.assertions.map((entry) => entry.createdAt), dayKeys);

  return [
    { text: labelLine },
    { text: `corrections: ${renderActivityBars(correctionCounts, "-")}` },
    { text: `learnings:   ${renderActivityBars(learningCounts, "#")}` },
    { text: `ratchet:     ${renderActivityBars(ratchetCounts, "*")}` },
  ];
}

function buildRecentDayKeys(memory: NonNullable<MissionControlSnapshot["memory"]>): readonly string[] {
  const timestamps = [
    ...memory.corrections.map((entry) => entry.createdAt),
    ...memory.rawLearnings.map((entry) => entry.sessionDate),
    ...memory.ratchetSuite.assertions.map((entry) => entry.createdAt),
    memory.compiledLearnings?.compiledAt ?? "",
  ].filter((value) => value.length > 0);
  const latestDate = timestamps.length > 0 ? new Date([...timestamps].sort().at(-1)!) : new Date();

  return Array.from({ length: 5 }, (_, index) => {
    const day = new Date(latestDate);
    day.setUTCDate(latestDate.getUTCDate() - (4 - index));
    return day.toISOString().slice(0, 10);
  });
}

function countEntriesByDay(timestamps: readonly string[], dayKeys: readonly string[]): readonly number[] {
  return dayKeys.map((dayKey) => timestamps.filter((timestamp) => timestamp.startsWith(dayKey)).length);
}

function countRecentEntries(timestamps: readonly string[]): number {
  const latest = timestamps.length > 0 ? new Date([...timestamps].sort().at(-1)!) : new Date();
  const cutoff = new Date(latest);
  cutoff.setUTCDate(latest.getUTCDate() - 6);
  return timestamps.filter((timestamp) => new Date(timestamp) >= cutoff).length;
}

function formatShortDay(dayKey: string): string {
  const parts = dayKey.split("-").map((part) => Number(part));
  if (parts.length !== 3) {
    return dayKey;
  }
  const year = parts[0];
  const month = parts[1];
  const day = parts[2];
  if (
    year === undefined
    || month === undefined
    || day === undefined
    || !Number.isFinite(year)
    || !Number.isFinite(month)
    || !Number.isFinite(day)
  ) {
    return dayKey;
  }
  const date = new Date(Date.UTC(year, month - 1, day));
  return date.toLocaleString("en-US", { month: "short", day: "numeric", timeZone: "UTC" });
}

function renderActivityBars(counts: readonly number[], marker: string): string {
  return counts
    .map((count) => marker.repeat(Math.max(1, count)).padEnd(6, " "))
    .join(" ");
}

function firstLine(text: string): string {
  return text.split("\n").map((line) => line.trim()).find((line) => line.length > 0) ?? "";
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

function formatTaskStatus(status: keyof typeof FEATURE_TASK_STATUS_LABEL): string {
  return FEATURE_TASK_STATUS_LABEL[status].toLowerCase();
}

export function isSelectableListModal(kind: AppState["modal"]["kind"]): kind is "feature-browser" | "handoffs" | "dependencies" | "memory" | "graph" | "agent-grid" | "dispatch" | "event-stream" | "task-board" | "timeline" {
  return kind === "feature-browser"
    || kind === "handoffs"
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
    case "handoffs":
      return { type: "open-handoffs" };
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
