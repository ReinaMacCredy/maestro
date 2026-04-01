/**
 * Frame composition -- renderFrame() and helpers.
 * Extracted from index.ts -- pure rendering, no side effects.
 */
import { Buffer } from "../terminal/buffer.js";
import { inset, type Rect } from "../terminal/layout.js";
import type { MissionControlSnapshot } from "../state/types.js";
import { createInitialState, type AppState } from "../state/reducer.js";
import { renderHeader } from "../panels/header.js";
import { renderStatusBar } from "../panels/status-bar.js";
import { renderFeatureDetail } from "../panels/feature-detail.js";
import { renderFeatureList } from "../panels/feature-list.js";
import { renderProgressLog } from "../panels/progress-log.js";
import { renderSessionSidebar } from "../panels/worker.js";
import { renderFooter } from "../panels/footer.js";
import {
  applyModalBackdrop,
  layoutModal,
  renderModal,
} from "../widgets/modal.js";
import { PALETTE } from "../theme.js";
import { BOX } from "../terminal/ansi.js";
import { buildModalOptions } from "./modal-builders.js";

export interface OnceFrameOptions {
  snapshot: MissionControlSnapshot;
}

/**
 * Render a single plain-text frame (for --once mode).
 */
export function renderOnceFrame(opts: OnceFrameOptions): string {
  const width = Math.min(process.stdout.columns || 120, 200);
  const minHeight = Math.max(opts.snapshot.features.length * 2 + 24, 36);
  const height = Math.max(process.stdout.rows || 0, minHeight);
  const buf = new Buffer(width, height);
  const state = createInitialState(opts.snapshot);
  renderFrame(buf, state, 0, 0);
  return process.stdout.isTTY ? buf.toAnsiString() : buf.toString();
}

export function renderFrame(
  buf: Buffer,
  state: AppState,
  animationFrame = 0,
  elapsedOffsetMs = 0,
): void {
  const snap = state.snapshot;
  const w = buf.width;
  const h = buf.height;
  if (w < 4 || h < 8) return;

  const borderStyle = { fg: PALETTE.border };
  const outerRect = { x: 0, y: 0, width: w, height: h };
  const innerRect = inset(outerRect, 1);
  buf.drawBorder(outerRect, borderStyle);

  const headerRect: Rect = { x: innerRect.x, y: innerRect.y, width: innerRect.width, height: 1 };
  const headerDividerY = headerRect.y + headerRect.height;
  const statusRect: Rect = { x: innerRect.x, y: headerDividerY + 1, width: innerRect.width, height: 2 };
  const statusDividerY = statusRect.y + statusRect.height;
  const bottomDividerY = h - 5;
  const spacerY = h - 4;
  const footerDividerY = h - 3;
  const footerRect: Rect = { x: innerRect.x, y: h - 2, width: innerRect.width, height: 1 };

  drawFullWidthDivider(buf, headerDividerY, borderStyle);
  drawFullWidthDivider(buf, statusDividerY, borderStyle);
  drawFullWidthDivider(buf, bottomDividerY, borderStyle);
  drawFullWidthDivider(buf, footerDividerY, borderStyle);
  buf.fillRect({ x: 1, y: spacerY, width: w - 2, height: 1 }, " ");

  const bodyY = statusDividerY + 1;
  const bodyBottomY = bottomDividerY - 1;
  const bodyHeight = Math.max(0, bodyBottomY - bodyY + 1);
    const bottomPaneHeight = Math.min(Math.max(6, Math.floor(bodyHeight * 0.26)), Math.max(6, bodyHeight - 5));
    const topBodyHeight = Math.max(4, bodyHeight - bottomPaneHeight - 1);
    const topBodyRect: Rect = { x: innerRect.x, y: bodyY, width: innerRect.width, height: topBodyHeight };
    const workerDividerY = topBodyRect.y + topBodyRect.height;
    const bottomBodyRect: Rect = {
      x: innerRect.x,
      y: workerDividerY + 1,
      width: innerRect.width,
      height: Math.max(0, bodyBottomY - workerDividerY),
    };

  drawFullWidthDivider(buf, workerDividerY, borderStyle);

  const splitOffset = clamp(Math.round(innerRect.width * (11 / 20)), 20, Math.max(20, innerRect.width - 18));
  const bodySplitX = clamp(innerRect.x + splitOffset, innerRect.x + 12, innerRect.x + innerRect.width - 13);
  drawVerticalDivider(buf, bodySplitX, statusDividerY, bottomDividerY, borderStyle);
  buf.set(statusDividerY, bodySplitX, BOX.teeDown, borderStyle);
  buf.set(workerDividerY, bodySplitX, BOX.cross, borderStyle);
  buf.set(bottomDividerY, bodySplitX, BOX.teeUp, borderStyle);

  const leftRect: Rect = {
    x: innerRect.x,
    y: topBodyRect.y,
    width: Math.max(0, bodySplitX - innerRect.x),
    height: topBodyRect.height,
  };
    const rightRect: Rect = {
      x: bodySplitX + 1,
      y: topBodyRect.y,
      width: Math.max(0, innerRect.x + innerRect.width - bodySplitX - 1),
      height: topBodyRect.height,
    };
    const featureListRect: Rect = rightRect;
    const timelineRect: Rect = {
      x: innerRect.x,
      y: bottomBodyRect.y,
      width: Math.max(0, bodySplitX - innerRect.x),
      height: bottomBodyRect.height,
    };
    const sessionRect: Rect = {
      x: bodySplitX + 1,
      y: bottomBodyRect.y,
      width: Math.max(0, innerRect.x + innerRect.width - bodySplitX - 1),
      height: bottomBodyRect.height,
    };

    renderHeader(buf, headerRect, snap, animationFrame);
    renderStatusBar(buf, statusRect, snap);
    renderFeatureDetail(buf, leftRect, snap, state.leftPaneMode, state.selectedFeatureIndex);
    renderFeatureList(buf, featureListRect, snap, state.selectedFeatureIndex, state.leftPaneMode === "preview");
    renderProgressLog(buf, timelineRect, snap.progressLog, snap, state.logScrollOffset);
    renderSessionSidebar(buf, sessionRect, snap);
    renderFooter(buf, footerRect, snap, state.copyMode);

    // Modal overlay
    const modalRect = getModalParentRect(w, h);
    if (state.modal.kind !== "none" && modalRect.width > 0 && modalRect.height > 0) {
      renderModalOverlay(buf, modalRect, state);
    }
  }

function drawFullWidthDivider(buf: Buffer, y: number, s: { fg: number }): void {
  drawHorizontalRange(buf, y, 0, buf.width - 1, s, BOX.teeRight, BOX.teeLeft);
}

function drawHorizontalRange(
  buf: Buffer,
  y: number,
  startX: number,
  endX: number,
  s: { fg: number },
  leftCap: string,
  rightCap: string,
): void {
  if (startX > endX || y < 0 || y >= buf.height) return;
  buf.set(y, startX, leftCap, s);
  for (let x = startX + 1; x < endX; x++) {
    buf.set(y, x, BOX.horizontal, s);
  }
  if (endX > startX) {
    buf.set(y, endX, rightCap, s);
  }
}

function drawVerticalDivider(
  buf: Buffer,
  x: number,
  startY: number,
  endY: number,
  s: { fg: number },
): void {
  for (let y = startY + 1; y < endY; y++) {
    buf.set(y, x, BOX.vertical, s);
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function renderModalOverlay(buf: Buffer, parentRect: Rect, state: AppState): void {
  const opts = buildModalOptions(state);
  if (!opts) return;
  applyModalBackdrop(buf);
  renderModal(buf, parentRect, opts);
}

export function getActiveModalLayout(width: number, height: number, state: AppState) {
  const opts = buildModalOptions(state);
  if (!opts || state.modal.kind === "none") return undefined;
  const parentRect = getModalParentRect(width, height);
  if (parentRect.width <= 0 || parentRect.height <= 0) return undefined;
  return layoutModal(parentRect, opts);
}

export function getModalParentRect(width: number, height: number): Rect {
  const innerRect = inset({ x: 0, y: 0, width, height }, 1);
  const headerDividerY = innerRect.y + 1;
  const statusRectY = headerDividerY + 1;
  const statusDividerY = statusRectY + 1;
  const bodyY = statusDividerY + 1;
  const footerDividerY = height - 3;
  return {
    x: innerRect.x,
    y: bodyY,
    width: innerRect.width,
    height: Math.max(0, footerDividerY - bodyY),
  };
}
