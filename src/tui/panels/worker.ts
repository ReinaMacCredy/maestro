/**
 * Lower pane -- Activity and Session sidebars.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../state/types.js";
import { sanitizeTerminalText } from "../../lib/sanitize.js";
import { FEATURE_STATUS_COLOR, FEATURE_STATUS_LABEL, PALETTE } from "../theme.js";
import { formatElapsed, truncate } from "../format.js";
import { shortenSessionId } from "../session-id.js";

export function renderWorkerPanel(
  buf: Buffer,
  rect: Rect,
  snap: MissionControlSnapshot,
  elapsedOffsetMs = 0,
): void {
  const y = rect.y;
  const w = rect.width;
  if (rect.height <= 0 || w <= 0) return;

  const splitOffset = clamp(Math.round(w * (11 / 20)), 20, Math.max(20, w - 18));
  const leftRect: Rect = {
    x: rect.x,
    y: rect.y,
    width: Math.max(0, splitOffset),
    height: rect.height,
  };
  const rightRect: Rect = {
    x: rect.x + splitOffset + 1,
    y: rect.y,
    width: Math.max(0, w - splitOffset - 1),
    height: rect.height,
  };

  renderActivityPane(buf, leftRect, snap);
  renderSessionPane(buf, rightRect, snap, elapsedOffsetMs);
}

export function renderSessionSidebar(
  buf: Buffer,
  rect: Rect,
  snap: MissionControlSnapshot,
): void {
  if (rect.width <= 0 || rect.height <= 0) return;
  buf.writeText(rect.y, rect.x + 1, "Session / Changes", { fg: PALETTE.brightWhite, bold: true });

  const session = snap.session;
  let row = rect.y + 2;
  const rows = session
    ? [
      { label: "Agent", value: session.agent ?? "--", style: "value" as const },
      { label: "Session", value: session.sessionId ? shortenSessionId(session.sessionId) : "--", style: "value" as const },
      { label: "Branch", value: session.branch, style: "value" as const },
      { label: "Changes", value: getChangesText(session), style: "changes" as const },
    ]
    : [
      { label: "Agent", value: "--", style: "muted" as const },
      { label: "Session", value: "--", style: "muted" as const },
      { label: "Branch", value: "--", style: "muted" as const },
      { label: "Changes", value: "no repo", style: "muted" as const },
    ];

  for (const entry of rows) {
    if (row >= rect.y + rect.height) return;
    if (entry.style === "changes") {
      writeChangesRow(buf, row, rect, entry.label, session);
    } else {
      writeLabeledRow(buf, row, rect, entry.label, entry.value, entry.style);
    }
    row++;
  }

  if (!session || session.workingTreeClean) {
    if (row < rect.y + rect.height) {
      writeLabeledRow(buf, row, rect, "Files", "no local edits", "muted");
    }
    return;
  }

  const fileChanges = session.fileChanges ?? session.changedFiles.map((path) => ({ path, kind: "modified" as const }));
  for (const [index, fileChange] of fileChanges.slice(0, Math.max(0, rect.height - (row - rect.y) - 1)).entries()) {
    if (row >= rect.y + rect.height) break;
    const label = index === 0 ? "Files" : "";
    const presentation = getFileChangePresentation(fileChange.kind);
    const labelWidth = 9;
    if (label) {
      buf.writeText(row, rect.x + 1, truncate(label, labelWidth), { fg: PALETTE.gray });
    }
    const valueX = rect.x + 1 + (label ? labelWidth + 1 : 0);
    buf.writeText(row, valueX, presentation.symbol, { fg: presentation.color });
    buf.writeText(row, valueX + 2, truncate(sanitizeTerminalText(fileChange.path), Math.max(0, rect.x + rect.width - valueX - 3)), {
      fg: presentation.color,
    });
    row++;
  }
}

function renderActivityPane(buf: Buffer, rect: Rect, snap: MissionControlSnapshot): void {
  if (rect.width <= 0 || rect.height <= 0) return;
  buf.writeText(rect.y, rect.x + 1, "Activity", { fg: PALETTE.brightWhite, bold: true });

  const lines = buildActivityRows(snap);
  let row = rect.y + 2;
  for (const line of lines) {
    if (row >= rect.y + rect.height) break;
    writeLabeledRow(buf, row, rect, line.label, line.value, line.style);
    row++;
  }
}

function renderSessionPane(
  buf: Buffer,
  rect: Rect,
  snap: MissionControlSnapshot,
  elapsedOffsetMs: number,
): void {
  if (rect.width <= 0 || rect.height <= 0) return;
  buf.writeText(rect.y, rect.x + 1, "Session", { fg: PALETTE.brightWhite, bold: true });

  const session = snap.session;
  const durationMs = snap.activeWorker
    ? snap.activeWorker.elapsedMs + elapsedOffsetMs
    : snap.elapsedMs + elapsedOffsetMs;
  let row = rect.y + 2;
  const rows = session
    ? [
        { label: "Duration", value: formatElapsed(durationMs), style: "value" as const },
        { label: "Branch", value: session.branch, style: "value" as const },
        { label: "Changes", value: getChangesText(session), style: "changes" as const },
        ...getFileRows(session),
      ]
    : [
      { label: "Duration", value: "--", style: "muted" as const },
      { label: "Branch", value: "--", style: "muted" as const },
      { label: "Changes", value: "no repo", style: "muted" as const },
      { label: "Files", value: "open a git repository", style: "muted" as const },
    ];

  for (const entry of rows) {
    if (row >= rect.y + rect.height) break;
    if (entry.style === "changes") {
      writeChangesRow(buf, row, rect, entry.label, session);
    } else {
      writeLabeledRow(buf, row, rect, entry.label, entry.value, entry.style);
    }
    row++;
  }
}

function buildActivityRows(
  snap: MissionControlSnapshot,
): Array<{ label: string; value: string; style: "title" | "meta" | "value" | "muted" }> {
  if (snap.mode === "home" && snap.home) {
    return [
      { label: "Task", value: snap.home.headline, style: "title" },
      { label: "Meta", value: "home · read-only", style: "muted" },
      { label: "State", value: snap.home.summary, style: "value" },
      {
        label: "Next",
        value: snap.home.actions[0]?.command ?? "Run maestro doctor",
        style: snap.home.actions.length > 0 ? "value" : "muted",
      },
      { label: "Scope", value: snap.home.locationLabel, style: "muted" },
    ];
  }

  if (snap.activeWorker) {
    const meta = `${snap.activeWorker.featureId} · ${FEATURE_STATUS_LABEL[snap.activeWorker.status]} · ${snap.activeWorker.workerType}`;
    const stateValue = getWorkerStateText(snap.activeWorker);
    const nextValue = getWorkerNextText(snap.activeWorker);
    return [
      { label: "Task", value: snap.activeWorker.featureTitle, style: "title" },
      { label: "Meta", value: meta, style: "meta" },
      {
        label: "State",
        value: stateValue,
        style: stateValue === "Waiting for first worker report" ? "muted" : "value",
      },
      {
        label: "Next",
        value: nextValue,
        style: nextValue === "Report update or status transition" ? "muted" : "value",
      },
      { label: "Scope", value: "Live on current feature", style: "value" },
    ];
  }

  if (snap.activeFeature) {
    const meta = `${snap.activeFeature.id} · ${FEATURE_STATUS_LABEL[snap.activeFeature.status]} · ${snap.activeFeature.workerType}`;
    const stateValue = snap.activeFeature.runtimeState === "recoverable"
      ? `Recovery ready · retry count ${snap.activeFeature.retryCount ?? 0}`
      : "Waiting to start next feature";
    const nextValue = snap.activeFeature.runtimeState === "recoverable"
      ? "Prompt generation can resume this feature"
      : "Open Features and choose a task to focus";
    return [
      { label: "Task", value: snap.activeFeature.title, style: "title" },
      { label: "Meta", value: meta, style: "meta" },
      { label: "State", value: stateValue, style: snap.activeFeature.runtimeState === "recoverable" ? "value" : "muted" },
      { label: "Next", value: nextValue, style: "value" },
      { label: "Scope", value: "Ready on current mission", style: "value" },
    ];
  }

  return [
    { label: "Task", value: "No active work", style: "title" },
    { label: "Meta", value: "mission · idle", style: "muted" },
    { label: "State", value: "No features in this mission yet", style: "muted" },
    { label: "Next", value: "Create or import work to populate Mission Control", style: "value" },
    { label: "Scope", value: "Read-only until work exists", style: "muted" },
  ];
}

function getChangesText(session: MissionControlSnapshot["session"]): string {
  if (!session || session.workingTreeClean) {
    return "clean";
  }
  const fileLabel = session.changedFiles.length === 1 ? "file" : "files";
  return `${session.changedFiles.length} ${fileLabel} · ${session.diffStat}`;
}

function getWorkerStateText(activeWorker: NonNullable<MissionControlSnapshot["activeWorker"]>): string {
  if (activeWorker.runtimeState === "failed") {
    return activeWorker.failureReason ?? "Worker runtime failed";
  }
  if (activeWorker.runtimeState === "stale") {
    return `Worker heartbeat stale · last seen ${formatElapsed(activeWorker.lastSeenAgeMs ?? 0)} ago`;
  }
  if (activeWorker.runtimeState === "recoverable") {
    return `Recovery ready · retry count ${activeWorker.retryCount ?? 0}`;
  }
  if (activeWorker.report?.salientSummary) {
    return activeWorker.report.salientSummary;
  }
  if (activeWorker.runtimeState === "live") {
    return "Worker runtime live";
  }
  return "Waiting for first worker report";
}

function getWorkerNextText(activeWorker: NonNullable<MissionControlSnapshot["activeWorker"]>): string {
  if (activeWorker.runtimeState === "failed") {
    return "Recovery review or manual retry";
  }
  if (activeWorker.runtimeState === "stale") {
    return "Recovery review or manual retry";
  }
  if (activeWorker.runtimeState === "recoverable") {
    return "Retry attempt can be scheduled";
  }
  return activeWorker.report?.whatWasImplemented ?? "Report update or status transition";
}

function getFileRows(
  session: MissionControlSnapshot["session"],
): Array<{ label: string; value: string; style: "value" | "muted" }> {
  if (!session || session.workingTreeClean || session.changedFiles.length === 0) {
    return [{ label: "Files", value: "no local edits", style: "muted" }];
  }

  const rows: Array<{ label: string; value: string; style: "value" | "muted" }> = session.changedFiles.slice(0, 2).map((filePath, index) => ({
    label: index === 0 ? "Files" : "",
    value: filePath,
    style: "value" as const,
  }));
  const remaining = session.changedFiles.length - rows.length;
  if (remaining > 0) {
    rows.push({ label: "", value: `+${remaining} more`, style: "muted" as const });
  }
  return rows;
}

function writeLabeledRow(
  buf: Buffer,
  row: number,
  rect: Rect,
  label: string,
  value: string,
  style: "title" | "meta" | "value" | "muted",
): void {
  const labelWidth = 9;
  const availableWidth = Math.max(0, rect.width - 3);
    if (label) {
      buf.writeText(row, rect.x + 1, truncate(label, labelWidth), { fg: PALETTE.gray });
    }
  const valueOffset = label ? labelWidth + 1 : 0;
  const valueX = rect.x + 1 + valueOffset;
  const valueWidth = Math.max(0, availableWidth - valueOffset);
  const cellStyle = getRowStyle(style);
  buf.writeText(row, valueX, truncate(sanitizeTerminalText(value), valueWidth), cellStyle);
}

function writeChangesRow(
  buf: Buffer,
  row: number,
  rect: Rect,
  label: string,
  session: MissionControlSnapshot["session"],
): void {
  const labelWidth = 9;
  const availableWidth = Math.max(0, rect.width - 3);
  if (label) {
    buf.writeText(row, rect.x + 1, truncate(label, labelWidth), { fg: PALETTE.dimGray });
  }

  const valueOffset = label ? labelWidth + 1 : 0;
  const valueX = rect.x + 1 + valueOffset;
  const valueWidth = Math.max(0, availableWidth - valueOffset);

    if (!session || session.workingTreeClean) {
      buf.writeText(row, valueX, truncate("clean", valueWidth), { fg: PALETTE.overlayHint });
      return;
    }

  const fileLabel = session.changedFiles.length === 1 ? "file" : "files";
  let col = valueX;
    col += buf.writeText(row, col, `${session.changedFiles.length} ${fileLabel}`, { fg: PALETTE.overlayHint });
  if (col >= valueX + valueWidth) return;

  col += buf.writeText(row, col, " · ", { fg: PALETTE.dimGray });
  if (col >= valueX + valueWidth) return;

  const diff = parseDiffStatParts(session.diffStat);
  col += buf.writeText(row, col, diff.added, { fg: PALETTE.green });
  if (col >= valueX + valueWidth) return;

  buf.writeText(row, col, ` ${diff.deleted}`, { fg: PALETTE.red });
}

function getRowStyle(style: "title" | "meta" | "value" | "muted") {
  switch (style) {
    case "title":
      return { fg: PALETTE.brightWhite, bold: true };
      case "meta":
        return { fg: PALETTE.yellow, bold: true };
      case "value":
        return { fg: PALETTE.overlayHint };
      default:
        return { fg: PALETTE.gray };
    }
  }

function getFileChangePresentation(
  kind: NonNullable<NonNullable<MissionControlSnapshot["session"]>["fileChanges"]>[number]["kind"],
) {
  switch (kind) {
    case "deleted":
      return { symbol: "-", color: PALETTE.red } as const;
    case "added":
    case "copied":
    case "untracked":
      return { symbol: "+", color: PALETTE.green } as const;
    case "conflicted":
      return { symbol: "!", color: PALETTE.red } as const;
    default:
      return { symbol: "~", color: PALETTE.yellow } as const;
  }
}

function parseDiffStatParts(diffStat: string): { added: string; deleted: string } {
  const match = diffStat.match(/(\+\d+)\s+(-\d+)/);
  if (!match) {
    return { added: "+0", deleted: "-0" };
  }
  return { added: match[1]!, deleted: match[2]! };
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}
