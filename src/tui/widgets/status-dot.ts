/**
 * Status dot widget -- colored indicator by entity status.
 */
import type { Buffer } from "../terminal/buffer.js";
import type { FeatureStatus, MissionStatus, MilestoneStatus, AssertionResult } from "../../domain/mission-types.js";
import {
  FEATURE_STATUS_COLOR,
  MISSION_STATUS_COLOR,
  MILESTONE_STATUS_COLOR,
  ASSERTION_RESULT_COLOR,
  featureDot,
  DOT_FILLED,
} from "../theme.js";

/** Render a feature status dot at (row, col). */
export function renderFeatureDot(buf: Buffer, row: number, col: number, status: FeatureStatus): void {
  buf.set(row, col, featureDot(status), { fg: FEATURE_STATUS_COLOR[status] });
}

/** Render a mission status dot at (row, col). */
export function renderMissionDot(buf: Buffer, row: number, col: number, status: MissionStatus): void {
  buf.set(row, col, DOT_FILLED, { fg: MISSION_STATUS_COLOR[status] });
}

/** Render a milestone status dot at (row, col). */
export function renderMilestoneDot(buf: Buffer, row: number, col: number, status: MilestoneStatus): void {
  buf.set(row, col, DOT_FILLED, { fg: MILESTONE_STATUS_COLOR[status] });
}

/** Render an assertion result dot at (row, col). */
export function renderAssertionDot(buf: Buffer, row: number, col: number, result: AssertionResult): void {
  buf.set(row, col, DOT_FILLED, { fg: ASSERTION_RESULT_COLOR[result] });
}
