/**
 * Left pane renderer for Mission Overview, task preview, and home overview.
 */
import type { Buffer, Cell } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot, TaskPreviewPane } from "../state/types.js";
import type { LeftPaneMode } from "../state/types.js";
import { FEATURE_TASK_STATUS_LABEL, MISSION_STATUS_LABEL, PALETTE } from "../theme.js";
import { truncate } from "../format.js";
import { shortenSessionId } from "../session-id.js";
const TREE_BULLET = "●";
const TREE_CHILD = "└─";

export function renderFeatureDetail(
  buf: Buffer,
  rect: Rect,
  snap: MissionControlSnapshot,
  mode: LeftPaneMode,
  selectedFeatureIndex: number,
): void {
  if (snap.mode === "home" && snap.home) {
    renderHomeOverview(buf, rect, snap);
    return;
  }

  if (mode === "overview") {
    renderMissionOverview(buf, rect, snap);
    return;
  }

  const preview = snap.taskPreviews?.[selectedFeatureIndex] ?? snap.activeFeature;
  if (!preview) {
    buf.writeText(rect.y + 1, rect.x + 1, "No task selected", { fg: PALETTE.dimGray });
    return;
  }

  renderTaskPreview(buf, rect, preview);
}

function renderTaskPreview(buf: Buffer, rect: Rect, preview: TaskPreviewPane): void {
  const w = rect.width - 2;
  let row = rect.y;
  const maxRow = rect.y + rect.height;
  const writers = createPanelWriters(buf, rect, w, maxRow, () => row, (nextRow) => {
    row = nextRow;
  });
    const { writeLine } = writers;
    const writeBullet = (text: string): void => {
      writers.writeBullet(text, { fg: PALETTE.overlayHint });
    };

  const writeSection = (title: string): void => {
    if (row >= maxRow) return;
    row++; // blank line before section
    if (row >= maxRow) return;
      writeLine(title, { fg: PALETTE.brightWhite, bold: true });
    };

  buf.writeText(row, rect.x + 1, "Focus / Preview", { fg: PALETTE.brightWhite, bold: true });
  row += 1;
  writeLine(preview.title, { fg: PALETTE.brightWhite, bold: true });

  writeKeyValue(writers, "id", preview.id);
  writeKeyValue(writers, "status", FEATURE_TASK_STATUS_LABEL[preview.status].toLowerCase());
  writeKeyValue(writers, "milestone", preview.milestoneTitle);
  writeKeyValue(writers, "worker", preview.workerType);
  writeKeyValue(writers, "agent", preview.agent ?? "--");
  writeKeyValue(writers, "session", preview.sessionId ? shortenSessionId(preview.sessionId) : "--");
  writeKeyValue(writers, "runtime", preview.runtimeState ?? "--");
  row++;

  writeKeyValue(
    writers,
    "blocked by",
    preview.blockedBy && preview.blockedBy.length > 0
      ? preview.blockedBy.map((item) => item.id).join(", ")
      : "none",
  );
  writeKeyValue(
    writers,
    "unblocks",
    preview.unblocks && preview.unblocks.length > 0
      ? preview.unblocks.map((item) => `${item.id} ${item.title}`).join(", ")
      : "none",
  );

  if (preview.description && row < maxRow - 2) {
    writeSection("Description");
      const descLines = preview.description.split("\n").slice(0, maxRow - row - 1);
      for (const line of descLines) {
        writeLine(line, { fg: PALETTE.overlayHint });
      }
    }
  }

function renderMissionOverview(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const overview = snap.missionOverview;
  if (!overview) {
    buf.writeText(rect.y + 1, rect.x + 1, "Mission Overview unavailable", { fg: PALETTE.dimGray });
    return;
  }

  const w = rect.width - 2;
  let row = rect.y;
  const maxRow = rect.y + rect.height;
  const writers = createPanelWriters(buf, rect, w, maxRow, () => row, (nextRow) => {
    row = nextRow;
  });
  const { writeLine } = writers;

  writeLine("Mission Overview", { fg: PALETTE.brightWhite, bold: true });
    writeLine(overview.missionLabel, { fg: PALETTE.brightWhite, bold: true });
    if (overview.totalCount === 0) {
      writeLine("No active work", { fg: PALETTE.brightWhite, bold: true });
      writeLine("Create or import work to populate Mission Control", { fg: PALETTE.overlayHint });
      return;
    }
  writeKeyValue(writers, "status", formatMissionOverviewStatus(overview.statusLabel));
  writeKeyValue(writers, "active", String(overview.activeCount));
  writeKeyValue(writers, "done", `${overview.doneCount} / ${overview.totalCount}`);
  writeKeyValue(writers, "blocked", String(overview.blockedCount));
  writeKeyValue(writers, "current", overview.currentMilestone ?? "--");
  writeKeyValue(writers, "gate", overview.gateLabel ?? "clear");
  writeKeyValue(
    writers,
    "agents",
    overview.agentSummary.length > 0
      ? overview.agentSummary.map((entry) => `${entry.agent}(${entry.count})`).join(" ")
      : "none",
  );

  if (row < maxRow - 3) {
    row++;
    writeLine("Dependency Map", { fg: PALETTE.brightWhite, bold: true });
    if (overview.dependencyMap.length === 0) {
      writeLine("No linked tasks yet", { fg: PALETTE.dimGray });
      return;
    }
    for (const entry of overview.dependencyMap) {
        writers.writeBullet(
          `${TREE_BULLET} ${entry.root.id} ${entry.root.title} [${FEATURE_TASK_STATUS_LABEL[entry.root.status]}]`,
          { fg: PALETTE.overlayHint },
        );
        if (entry.primaryDependent) {
          writeLine(
          `${TREE_CHILD} ${truncate(
            `${entry.primaryDependent.id} ${entry.primaryDependent.title} [${formatDependencyLinkLabel(
              entry.primaryDependent.status,
              entry.primaryDependentBlockedByCount,
              )}]${entry.hiddenDependentCount > 0 ? ` +${entry.hiddenDependentCount} more` : ""}`,
              Math.max(0, w - 3),
            )}`,
            { fg: PALETTE.overlayHint },
          );
        }
      }
  }
}

function renderHomeOverview(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  const home = snap.home!;
  const w = rect.width - 2;
  let row = rect.y;
  const maxRow = rect.y + rect.height;
  const { writeLine, writeBullet } = createPanelWriters(buf, rect, w, maxRow, () => row, (nextRow) => {
    row = nextRow;
  });

    writeLine("Overview", { fg: PALETTE.brightWhite, bold: true });
    writeLine(home.headline, { fg: PALETTE.brightWhite });
    writeLine(home.summary, { fg: PALETTE.overlayHint });
  row++;

  writeLine("Workspace", { fg: PALETTE.brightWhite, bold: true });
  writeBullet(home.locationLabel);
  writeBullet(`${home.pendingHandoffs.length} pending handoff${home.pendingHandoffs.length === 1 ? "" : "s"}`);
  writeBullet(`${home.actions.length} suggested next step${home.actions.length === 1 ? "" : "s"}`);
}

function createPanelWriters(
  buf: Buffer,
  rect: Rect,
  width: number,
  maxRow: number,
  getRow: () => number,
  setRow: (row: number) => void,
): {
  writeLine: (text: string, style?: Partial<Cell>) => void;
  writeBullet: (text: string, style?: Partial<Cell>) => void;
} {
  const writeLine = (text: string, style?: Partial<Cell>): void => {
    const row = getRow();
    if (row >= maxRow) return;
    buf.writeText(row, rect.x + 1, truncate(text, width), style);
    setRow(row + 1);
  };

  const writeBullet = (text: string, style?: Partial<Cell>): void => {
    const row = getRow();
    if (row >= maxRow) return;
      buf.writeText(row, rect.x + 1, "  ", { fg: PALETTE.dimGray });
      buf.writeText(row, rect.x + 3, truncate(text, width - 2), style ?? { fg: PALETTE.overlayHint });
      setRow(row + 1);
    };

  return { writeLine, writeBullet };
}

function writeKeyValue(
  writers: ReturnType<typeof createPanelWriters>,
  label: string,
  value: string,
): void {
  writers.writeLine(`${label.padEnd(11, " ")}${value}`, { fg: PALETTE.overlayHint });
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
