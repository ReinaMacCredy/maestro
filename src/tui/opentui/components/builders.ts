import { TextAttributes } from "@opentui/core";

import type { AppState } from "../../state/reducer.js";
import type {
  MissionControlEvent,
  MissionControlFeatureRow,
  MissionControlMilestoneRow,
  MissionControlRuntimeProcessRow,
  MissionControlSnapshot,
  MissionControlWorkerPane,
  TaskPreviewPane,
} from "../../state/types.js";
import { getMissionControlCommandSpecs } from "../../state/mission-control-commands.js";
import { FEATURE_STATUS_LABEL, FEATURE_TASK_STATUS_LABEL, MISSION_STATUS_LABEL } from "../../theme.js";
import { formatAge, formatElapsed, formatTokens, truncate } from "../../format.js";
import { getHeaderDotsFrame } from "../../shared/header-animation.js";
import type { ModalOptions } from "../../shared/modal-model.js";
import { shortenSessionId } from "../../session-id.js";
import { buildModalOptions } from "../../app/modal-builders.js";

const MAX_HINT_WIDTH = 44;

export const OPEN_TUI_THEME = {
  pageBg: "#0b1118",
  panelBg: "#131b24",
  panelBgElevated: "#17212d",
  headerBg: "#0e151d",
  muted: "#8fa2b7",
  text: "#f5f7fa",
  accent: "#ffb454",
  success: "#7ee787",
  warning: "#ffd166",
  danger: "#ff7b72",
  info: "#79c0ff",
  selectionBg: "#1f4b99",
  selectionFg: "#ffffff",
} as const;

export interface MissionControlTheme {
  readonly pageBg?: string;
  readonly panelBg?: string;
  readonly headerBg?: string;
  readonly modalBg: string;
  readonly modalPanelBg: string;
  readonly muted: string;
  readonly text: string;
  readonly accent: string;
  readonly success: string;
  readonly warning: string;
  readonly danger: string;
  readonly info: string;
  readonly selectionBg: string;
  readonly selectionFg: string;
}

export function resolveMissionControlTheme(snapshot: MissionControlSnapshot): MissionControlTheme {
  const backgroundMode = snapshot.configSummary?.backgroundMode ?? "solid";
  if (backgroundMode === "terminal") {
    return {
      ...OPEN_TUI_THEME,
      pageBg: undefined,
      panelBg: undefined,
      headerBg: undefined,
      modalBg: OPEN_TUI_THEME.panelBgElevated,
      modalPanelBg: OPEN_TUI_THEME.panelBg,
    };
  }

  return {
    ...OPEN_TUI_THEME,
    modalBg: OPEN_TUI_THEME.panelBgElevated,
    modalPanelBg: OPEN_TUI_THEME.panelBg,
  };
}

export interface UiLine {
  readonly text: string;
  readonly fg?: string;
  readonly bg?: string;
  readonly attributes?: number;
}

export interface StatusStripModel {
  readonly primaryLeft: UiLine;
  readonly primaryRight?: UiLine;
  readonly secondaryLeft?: UiLine;
  readonly secondaryRight?: UiLine;
}

export interface FooterModel {
  readonly left: string;
  readonly right: string;
}

export interface ScreenLayout {
  readonly innerWidth: number;
  readonly innerHeight: number;
  readonly headerHeight: number;
  readonly statusHeight: number;
  readonly footerHeight: number;
  readonly bodyHeight: number;
  readonly stacked: boolean;
  readonly mainWidth: number;
  readonly sideWidth: number;
  readonly leftTopHeight: number;
  readonly leftBottomHeight: number;
  readonly rightTopHeight: number;
  readonly rightBottomHeight: number;
  readonly stackedHeights: readonly [number, number, number, number];
  readonly modalWidth: number;
  readonly modalHeight: number;
}

export interface ModalParentRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export function computeScreenLayout(
  width: number,
  height: number,
  snapshot: MissionControlSnapshot,
): ScreenLayout {
  const innerWidth = Math.max(20, width - 2);
  const innerHeight = Math.max(8, height - 2);
  const headerHeight = 1;
  const statusHeight = 2;
  const footerHeight = 1;
  const bodyHeight = Math.max(4, innerHeight - headerHeight - statusHeight - footerHeight);
  const stacked = width < 96;

  if (stacked) {
    const gapBudget = 3;
    const usableHeight = Math.max(4, bodyHeight - gapBudget);
    const focusHeight = Math.max(6, Math.floor(usableHeight * 0.34));
    const listHeight = Math.max(5, Math.floor(usableHeight * 0.21));
    const logHeight = Math.max(5, Math.floor(usableHeight * 0.2));
    const sessionHeight = Math.max(4, usableHeight - focusHeight - listHeight - logHeight);

    return {
      innerWidth,
      innerHeight,
      headerHeight,
      statusHeight,
      footerHeight,
      bodyHeight,
      stacked,
      mainWidth: innerWidth,
      sideWidth: innerWidth,
      leftTopHeight: focusHeight,
      leftBottomHeight: logHeight,
      rightTopHeight: listHeight,
      rightBottomHeight: sessionHeight,
      stackedHeights: [focusHeight, listHeight, logHeight, sessionHeight],
      modalWidth: clamp(Math.floor(innerWidth * 0.88), 58, Math.max(58, innerWidth - 4)),
      modalHeight: clamp(Math.floor(innerHeight * 0.72), 16, Math.max(16, innerHeight - 4)),
    };
  }

  const mainRatio = snapshot.mode === "home" ? 0.58 : 0.62;
  const mainWidth = clamp(Math.floor(innerWidth * mainRatio), 34, Math.max(34, innerWidth - 29));
  const sideWidth = Math.max(28, innerWidth - mainWidth - 1);
  const leftSplitRatio = snapshot.mode === "home" ? 0.60 : 0.68;
  const rightSplitRatio = 0.55;
  const usableColumnHeight = Math.max(4, bodyHeight - 1);
  const leftTopHeight = clamp(Math.floor(usableColumnHeight * leftSplitRatio), 8, Math.max(8, usableColumnHeight - 6));
  const leftBottomHeight = Math.max(5, usableColumnHeight - leftTopHeight);
  const rightTopHeight = clamp(Math.floor(usableColumnHeight * rightSplitRatio), 7, Math.max(7, usableColumnHeight - 5));
  const rightBottomHeight = Math.max(4, usableColumnHeight - rightTopHeight);

  return {
    innerWidth,
    innerHeight,
    headerHeight,
    statusHeight,
    footerHeight,
    bodyHeight,
    stacked,
    mainWidth,
    sideWidth,
    leftTopHeight,
    leftBottomHeight,
    rightTopHeight,
    rightBottomHeight,
    stackedHeights: [leftTopHeight, rightTopHeight, leftBottomHeight, rightBottomHeight],
    modalWidth: clamp(Math.floor(innerWidth * 0.78), 60, Math.max(60, innerWidth - 6)),
    modalHeight: clamp(Math.floor(innerHeight * 0.72), 18, Math.max(18, innerHeight - 4)),
  };
}

export function getModalParentRect(layout: ScreenLayout): ModalParentRect {
  return {
    x: Math.max(1, Math.floor((layout.innerWidth - layout.modalWidth) / 2)),
    y: Math.max(1, Math.floor((layout.innerHeight - layout.modalHeight) / 2)),
    width: layout.modalWidth,
    height: layout.modalHeight,
  };
}

export function buildHeaderModel(snapshot: MissionControlSnapshot, animationFrame = 0): {
  readonly left: UiLine;
  readonly right: UiLine;
} {
  const dots = getHeaderDotsFrame(snapshot, animationFrame);
  const leftText = `${dots} Mission Control`;
  const rightParts = [`TIME ${formatElapsed(snapshot.elapsedMs)}`];

  if (snapshot.tokenCounters) {
    rightParts.push(`Input ${formatTokens(snapshot.tokenCounters.input)}`);
    rightParts.push(`Cached ${formatTokens(snapshot.tokenCounters.cached)}`);
    rightParts.push(`Output ${formatTokens(snapshot.tokenCounters.output)}`);
  }

  return {
    left: { text: leftText, fg: OPEN_TUI_THEME.accent, attributes: TextAttributes.BOLD },
    right: { text: rightParts.join("  ·  "), fg: OPEN_TUI_THEME.muted },
  };
}

export function buildStatusStripModel(snapshot: MissionControlSnapshot): StatusStripModel {
  if (snapshot.mode === "home" && snapshot.home) {
    const failed = snapshot.home.checks.filter((check) => check.status === "fail").length;
    const warned = snapshot.home.checks.filter((check) => check.status === "warn").length;
    const summary = [
      `${snapshot.home.checks.length} checks`,
      `${snapshot.home.actions.length} next`,
      snapshot.home.pendingHandoffs.length > 0 ? `${snapshot.home.pendingHandoffs.length} handoffs` : undefined,
    ].filter(Boolean).join("  ·  ");

    const statusColor = failed > 0
      ? OPEN_TUI_THEME.danger
      : warned > 0
        ? OPEN_TUI_THEME.warning
        : OPEN_TUI_THEME.info;

    return {
      primaryLeft: {
        text: `HOME  ${snapshot.home.headline}`,
        fg: statusColor,
        attributes: TextAttributes.BOLD,
      },
      primaryRight: { text: summary, fg: OPEN_TUI_THEME.muted },
      secondaryLeft: { text: snapshot.home.summary, fg: OPEN_TUI_THEME.text },
      secondaryRight: { text: snapshot.home.locationLabel, fg: OPEN_TUI_THEME.muted },
    };
  }

  const label = MISSION_STATUS_LABEL[snapshot.effectiveStatus] ?? snapshot.effectiveStatus.toUpperCase();
  const milestone = getActiveMilestone(snapshot);
  const primaryRightParts = [`${snapshot.statusProgress.completed}/${snapshot.statusProgress.total} done`];
  if (snapshot.statusProgress.inFlight > 0) primaryRightParts.push(`${snapshot.statusProgress.inFlight} active`);
  if (snapshot.statusProgress.blocked > 0) primaryRightParts.push(`${snapshot.statusProgress.blocked} blocked`);
  if (snapshot.statusProgress.queued > 0) primaryRightParts.push(`${snapshot.statusProgress.queued} queued`);

  return {
    primaryLeft: {
      text: milestone
        ? `${label}  ${milestone.title}`
        : label,
      fg: missionStatusColor(snapshot.effectiveStatus),
      attributes: TextAttributes.BOLD,
    },
    primaryRight: {
      text: primaryRightParts.join("  ·  "),
      fg: OPEN_TUI_THEME.text,
      attributes: TextAttributes.BOLD,
    },
    secondaryLeft: {
      text: `Milestone: ${milestone?.title ?? "--"}`,
      fg: OPEN_TUI_THEME.muted,
    },
    secondaryRight: {
      text: `Gate: ${snapshot.gateBlocked ? snapshot.gateLabel ?? "blocked" : "clear"}`,
      fg: snapshot.gateBlocked ? OPEN_TUI_THEME.danger : OPEN_TUI_THEME.muted,
      attributes: snapshot.gateBlocked ? TextAttributes.BOLD : undefined,
    },
  };
}

export function buildFocusLines(
  state: AppState,
  contentWidth: number,
  contentHeight: number,
): readonly UiLine[] {
  if (state.snapshot.mode === "home" && state.snapshot.home) {
    const lines: UiLine[] = [
      boldLine(state.snapshot.home.headline),
      normalLine(state.snapshot.home.summary),
      blankLine(),
      sectionLine("Workspace"),
      bulletLine(state.snapshot.home.locationLabel),
      bulletLine(`${state.snapshot.home.pendingHandoffs.length} pending handoff${plural(state.snapshot.home.pendingHandoffs.length)}`),
      bulletLine(`${state.snapshot.home.actions.length} suggested next step${plural(state.snapshot.home.actions.length)}`),
    ];
    return clampLines(lines, contentWidth, contentHeight);
  }

  if (state.leftPaneMode === "overview") {
    const overview = state.snapshot.missionOverview;
    if (!overview) {
      return clampLines([
        boldLine(state.snapshot.missionTitle),
        keyValueLine("mission", state.snapshot.missionId),
        keyValueLine("status", MISSION_STATUS_LABEL[state.snapshot.effectiveStatus].toLowerCase()),
        blankLine(),
        mutedLine("Mission Overview unavailable"),
      ], contentWidth, contentHeight);
    }

    const lines: UiLine[] = [
      boldLine(overview.missionLabel),
      keyValueLine("status", formatMissionOverviewStatus(overview.statusLabel)),
      keyValueLine("active", String(overview.activeCount)),
      keyValueLine("done", `${overview.doneCount} / ${overview.totalCount}`),
      keyValueLine("blocked", String(overview.blockedCount)),
      keyValueLine("current", overview.currentMilestone ?? "--"),
      keyValueLine("gate", overview.gateLabel ?? "clear"),
      keyValueLine(
        "agents",
        overview.agentSummary.length > 0
          ? overview.agentSummary.map((entry) => `${entry.agent}(${entry.count})`).join(" ")
          : "none",
      ),
      blankLine(),
      sectionLine("Dependency Map"),
    ];

    if (overview.dependencyMap.length === 0) {
      lines.push(mutedLine("No linked tasks yet"));
    } else {
      for (const entry of overview.dependencyMap) {
        lines.push(normalLine(`● ${entry.root.id} ${entry.root.title} [${FEATURE_TASK_STATUS_LABEL[entry.root.status]}]`, OPEN_TUI_THEME.info));
        if (entry.primaryDependent) {
          const suffix = entry.hiddenDependentCount > 0 ? ` +${entry.hiddenDependentCount} more` : "";
          lines.push(
            normalLine(
              `└─ ${entry.primaryDependent.id} ${truncate(entry.primaryDependent.title, Math.max(0, contentWidth - 20))} [${formatDependencyLinkLabel(entry.primaryDependent.status, entry.primaryDependentBlockedByCount)}]${suffix}`,
            ),
          );
        }
      }
    }

    return clampLines(lines, contentWidth, contentHeight);
  }

  const preview = state.snapshot.taskPreviews?.[state.selectedFeatureIndex] ?? state.snapshot.activeFeature;
  if (!preview) {
    return clampLines([mutedLine("No task selected")], contentWidth, contentHeight);
  }

  const lines: UiLine[] = [
    boldLine(preview.title),
    keyValueLine("id", preview.id),
    keyValueLine("status", FEATURE_TASK_STATUS_LABEL[preview.status].toLowerCase()),
    keyValueLine("milestone", preview.milestoneTitle),
    keyValueLine("worker", preview.workerType),
    keyValueLine("agent", preview.agent ?? "--"),
    keyValueLine("session", preview.sessionId ? shortenSessionId(preview.sessionId) : "--"),
    keyValueLine("runtime", preview.runtimeState ?? "--"),
    blankLine(),
    keyValueLine(
      "blocked by",
      preview.blockedBy && preview.blockedBy.length > 0
        ? preview.blockedBy.map((item) => item.id).join(", ")
        : "none",
    ),
    keyValueLine(
      "unblocks",
      preview.unblocks && preview.unblocks.length > 0
        ? preview.unblocks.map((item) => `${item.id} ${item.title}`).join(", ")
        : "none",
    ),
  ];

  if (preview.description) {
    lines.push(blankLine(), sectionLine("Description"));
    for (const line of preview.description.split("\n").filter(Boolean)) {
      lines.push(normalLine(line));
    }
  }

  return clampLines(lines, contentWidth, contentHeight);
}

export function buildFeatureListLines(
  state: AppState,
  contentWidth: number,
  contentHeight: number,
): readonly UiLine[] {
  if (state.snapshot.mode === "home" && state.snapshot.home) {
    const okCount = state.snapshot.home.checks.filter((check) => check.status === "ok").length;
    const lines: UiLine[] = [sectionLine(`Health  ${okCount}/${state.snapshot.home.checks.length} ok`), blankLine()];
    for (const check of state.snapshot.home.checks) {
      const marker = check.status === "ok" ? "●" : check.status === "warn" ? "!" : "x";
      lines.push(
        lineWithTone(
          `${marker} ${check.message}`,
          check.status === "ok" ? "success" : check.status === "warn" ? "warning" : "danger",
        ),
      );
    }
    return clampLines(lines, contentWidth, contentHeight);
  }

  const lines: UiLine[] = [sectionLine(`Tasks  ${state.snapshot.featureProgress.done}/${state.snapshot.featureProgress.total}`), blankLine()];
  for (const [index, feature] of state.snapshot.features.entries()) {
    lines.push(buildFeatureRow(feature, index === state.selectedFeatureIndex, contentWidth));
  }
  return clampLines(lines, contentWidth, contentHeight);
}

export function buildLogLines(
  state: AppState,
  contentWidth: number,
  contentHeight: number,
): readonly UiLine[] {
  if (state.snapshot.mode === "home" && state.snapshot.home) {
    const lines: UiLine[] = [];
    if (state.snapshot.home.pendingHandoffs.length === 0) {
      lines.push(mutedLine("No pending handoffs"));
    } else {
      for (const handoff of state.snapshot.home.pendingHandoffs) {
        lines.push(normalLine(`${handoff.id} · ${handoff.agent}`, OPEN_TUI_THEME.muted));
        lines.push(normalLine(handoff.message));
        lines.push(blankLine());
      }
    }
    return clampLines(lines, contentWidth, contentHeight);
  }

  if (state.snapshot.progressLog.length === 0) {
    return clampLines([mutedLine("No events yet")], contentWidth, contentHeight);
  }

  const nowMs = Date.now();
  const visibleEvents = state.logScrollOffset > 0
    ? state.snapshot.progressLog.slice(state.logScrollOffset)
    : state.snapshot.progressLog;
  const lines: UiLine[] = [];
  for (const event of visibleEvents) {
    const age = formatAge(new Date(event.timestamp).getTime(), nowMs).padStart(8, " ");
    lines.push(normalLine(`${age}  ${event.title}`, eventTone(event)));
  }
  return clampLines(lines, contentWidth, contentHeight);
}

export function buildSessionLines(
  state: AppState,
  contentWidth: number,
  contentHeight: number,
  elapsedOffsetMs = 0,
): readonly UiLine[] {
  const lines: UiLine[] = buildActivitySummary(state.snapshot, elapsedOffsetMs);
  lines.push(blankLine(), sectionLine("Session"));

  const session = state.snapshot.session;
  if (!session) {
    lines.push(mutedLine("No session bound"));
    return clampLines(lines, contentWidth, contentHeight);
  }

  lines.push(
    keyValueLine("agent", session.agent ?? "--"),
    keyValueLine("transport", session.transport ?? "--"),
    keyValueLine(session.transport === "a2a" ? "handle" : "session", session.sessionId ? shortenSessionId(session.sessionId) : "--"),
    keyValueLine("branch", session.branch),
    keyValueLine("changes", getChangesText(session)),
  );

  const fileChanges = session.fileChanges ?? session.changedFiles.map((path) => ({ path, kind: "modified" as const }));
  if (fileChanges.length > 0) {
    lines.push(blankLine(), sectionLine("Files"));
    for (const fileChange of fileChanges.slice(0, Math.max(0, contentHeight - lines.length - 1))) {
      lines.push(normalLine(`${fileChange.kind === "added" ? "+" : fileChange.kind === "deleted" ? "-" : "~"} ${fileChange.path}`, OPEN_TUI_THEME.muted));
    }
  }

  return clampLines(lines, contentWidth, contentHeight);
}

export function buildFooterModel(snapshot: MissionControlSnapshot, copyMode: boolean): FooterModel {
  const commandSpecs = getMissionControlCommandSpecs(snapshot.mode);
  const left = commandSpecs
    .filter((command) => command.key.length === 1)
    .map((command) => `${command.key} ${command.label}`)
    .concat(copyMode ? ["Esc Copy Off"] : ["Ctrl+Y Copy"])
    .join("  ");
  const exitCommand = commandSpecs.find((command) => command.id === "exit");
  return {
    left: truncate(copyMode ? "COPY MODE ACTIVE  drag-select enabled" : left, MAX_HINT_WIDTH * 2),
    right: `Ctrl+P Commands  ${exitCommand?.key ?? "Ctrl+T"} ${exitCommand?.label ?? "Exit"}`,
  };
}

export function buildModalModel(state: AppState): ModalOptions | undefined {
  return buildModalOptions(state);
}

function buildFeatureRow(feature: MissionControlFeatureRow, selected: boolean, contentWidth: number): UiLine {
  const statusLabel = FEATURE_TASK_STATUS_LABEL[feature.status].toUpperCase().padEnd(8, " ");
  const blocked = feature.status === "blocked" && feature.blockedByLabel ? ` by ${feature.blockedByLabel}` : "";
  const text = `${selected ? ">" : " "} ${statusLabel} ${feature.id} ${truncate(feature.title, Math.max(0, contentWidth - statusLabel.length - feature.id.length - blocked.length - 6))}${blocked}`;
  return {
    text,
    fg: selected ? OPEN_TUI_THEME.selectionFg : featureTone(feature.status),
    bg: selected ? OPEN_TUI_THEME.selectionBg : undefined,
    attributes: selected ? TextAttributes.BOLD : undefined,
  };
}

function buildActivitySummary(
  snapshot: MissionControlSnapshot,
  elapsedOffsetMs: number,
): UiLine[] {
  if (snapshot.mode === "home" && snapshot.home) {
    return [
      boldLine(snapshot.home.headline),
      normalLine(snapshot.home.summary),
      keyValueLine("next", snapshot.home.actions[0]?.command ?? "Run maestro doctor"),
      keyValueLine("scope", snapshot.home.locationLabel),
    ];
  }

  if (snapshot.activeWorker) {
    return workerSummaryLines(snapshot.activeWorker, elapsedOffsetMs);
  }

  if (snapshot.activeFeature) {
    const lines: UiLine[] = [
      boldLine(snapshot.activeFeature.title),
      normalLine(
        `${snapshot.activeFeature.id} · ${FEATURE_STATUS_LABEL[snapshot.activeFeature.status]} · ${snapshot.activeFeature.workerType}`,
        OPEN_TUI_THEME.muted,
      ),
      keyValueLine(
        "state",
        snapshot.activeFeature.runtimeState === "recoverable"
          ? `Recovery ready · retry count ${snapshot.activeFeature.retryCount ?? 0}`
          : "Waiting to start next feature",
      ),
      keyValueLine(
        "next",
        snapshot.activeFeature.runtimeState === "recoverable"
          ? "Prompt generation can resume this feature"
          : "Open Tasks and choose a feature to focus",
      ),
    ];
    return lines;
  }

  return [
    boldLine("No active work"),
    mutedLine("mission · idle"),
    keyValueLine("state", "No features in this mission yet"),
    keyValueLine("next", "Create or import work to populate Mission Control"),
  ];
}

function workerSummaryLines(worker: MissionControlWorkerPane, elapsedOffsetMs: number): UiLine[] {
  const lines: UiLine[] = [
    boldLine(worker.featureTitle),
    normalLine(`${worker.featureId} · ${FEATURE_STATUS_LABEL[worker.status]} · ${worker.workerType}`, OPEN_TUI_THEME.muted),
    keyValueLine("state", getWorkerStateText(worker)),
    keyValueLine("next", getWorkerNextText(worker)),
    keyValueLine("duration", formatElapsed(worker.elapsedMs + elapsedOffsetMs)),
  ];
  return lines;
}

function getWorkerStateText(activeWorker: MissionControlWorkerPane): string {
  if (activeWorker.runtimeState === "failed") {
    return activeWorker.failureReason ?? "Worker runtime failed";
  }
  if (activeWorker.runtimeState === "stale") {
    return `Worker heartbeat stale · last seen ${formatElapsed(activeWorker.lastSeenAgeMs ?? 0)} ago`;
  }
  if (activeWorker.runtimeState === "recoverable") {
    return `Recovery ready · retry count ${activeWorker.retryCount ?? 0}`;
  }
  if (activeWorker.runtimeState === "live" && activeWorker.currentActivity) {
    return activeWorker.currentActivity;
  }
  if (activeWorker.report?.salientSummary) {
    return activeWorker.report.salientSummary;
  }
  if (activeWorker.runtimeState === "live") {
    return typeof activeWorker.lastOutputAgeMs === "number"
      ? `Worker runtime live · last output ${formatElapsed(activeWorker.lastOutputAgeMs)} ago`
      : "Worker runtime live";
  }
  return "Waiting for first worker report";
}

function getWorkerNextText(activeWorker: MissionControlWorkerPane): string {
  if (activeWorker.runtimeState === "failed") {
    return "Inspect runtime and resolve the worker failure";
  }
  if (activeWorker.runtimeState === "recoverable") {
    return "Resume this feature or review the generated recovery state";
  }
  if (activeWorker.runtimeState === "stale") {
    return "Wait for the next heartbeat or restart the worker";
  }
  return "Report update or status transition";
}

function getChangesText(session: NonNullable<MissionControlSnapshot["session"]>): string {
  if (session.workingTreeClean) {
    return "clean";
  }
  const fileLabel = session.changedFiles.length === 1 ? "file" : "files";
  return `${session.changedFiles.length} ${fileLabel} · ${session.diffStat}`;
}

function getActiveMilestone(snapshot: MissionControlSnapshot): MissionControlMilestoneRow | undefined {
  return snapshot.milestones.find((milestone) =>
    milestone.status === "executing" || milestone.status === "validating");
}

function missionStatusColor(status: MissionControlSnapshot["effectiveStatus"]): string {
  switch (status) {
    case "executing":
      return OPEN_TUI_THEME.accent;
    case "validating":
      return OPEN_TUI_THEME.warning;
    case "completed":
      return OPEN_TUI_THEME.success;
    case "failed":
    case "rejected":
      return OPEN_TUI_THEME.danger;
    case "approved":
      return OPEN_TUI_THEME.info;
    default:
      return OPEN_TUI_THEME.muted;
  }
}

function featureTone(status: MissionControlFeatureRow["status"]): string {
  switch (status) {
    case "done":
      return OPEN_TUI_THEME.success;
    case "review":
      return OPEN_TUI_THEME.warning;
    case "blocked":
      return OPEN_TUI_THEME.danger;
    default:
      return OPEN_TUI_THEME.text;
  }
}

function eventTone(event: MissionControlEvent): string {
  if (event.kind === "worker") return OPEN_TUI_THEME.info;
  if (event.kind === "checkpoint") return OPEN_TUI_THEME.warning;
  if (event.kind === "feature" && event.title.toLowerCase().includes("blocked")) return OPEN_TUI_THEME.danger;
  if (event.kind === "feature") return OPEN_TUI_THEME.success;
  return OPEN_TUI_THEME.text;
}

function formatMissionOverviewStatus(statusLabel: string): string {
  const missionStatus = statusLabel as keyof typeof MISSION_STATUS_LABEL;
  return MISSION_STATUS_LABEL[missionStatus]?.toLowerCase() ?? statusLabel;
}

function formatDependencyLinkLabel(
  status: TaskPreviewPane["status"],
  blockedByCount?: number,
): string {
  if (status !== "blocked") {
    return FEATURE_TASK_STATUS_LABEL[status];
  }
  if (!blockedByCount || blockedByCount <= 1) {
    return FEATURE_TASK_STATUS_LABEL.blocked;
  }
  return `${FEATURE_TASK_STATUS_LABEL.blocked} by ${blockedByCount}`;
}

function keyValueLine(label: string, value: string): UiLine {
  return normalLine(`${label.padEnd(11, " ")}${value}`, OPEN_TUI_THEME.muted);
}

function sectionLine(text: string): UiLine {
  return {
    text,
    fg: OPEN_TUI_THEME.accent,
    attributes: TextAttributes.BOLD,
  };
}

function bulletLine(text: string): UiLine {
  return normalLine(`- ${text}`, OPEN_TUI_THEME.muted);
}

function boldLine(text: string): UiLine {
  return {
    text,
    fg: OPEN_TUI_THEME.text,
    attributes: TextAttributes.BOLD,
  };
}

function mutedLine(text: string): UiLine {
  return normalLine(text, OPEN_TUI_THEME.muted);
}

function lineWithTone(
  text: string,
  tone: "success" | "warning" | "danger",
): UiLine {
  const fg = tone === "success"
    ? OPEN_TUI_THEME.success
    : tone === "warning"
      ? OPEN_TUI_THEME.warning
      : OPEN_TUI_THEME.danger;
  return normalLine(text, fg);
}

function normalLine(text: string, fg = OPEN_TUI_THEME.text): UiLine {
  return { text, fg };
}

function blankLine(): UiLine {
  return { text: "" };
}

function clampLines(lines: readonly UiLine[], width: number, height: number): readonly UiLine[] {
  return lines.slice(0, Math.max(0, height)).map((line) => ({
    ...line,
    text: truncate(line.text, Math.max(0, width)),
  }));
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function plural(count: number): string {
  return count === 1 ? "" : "s";
}
