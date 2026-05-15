/**
 * TUI theme -- status-to-color maps and palette constants.
 * Uses 256-color indices for broad terminal compatibility.
 */
import type { MissionStatus, FeatureStatus, MilestoneStatus, AssertionResult, MilestoneKind, MilestoneProfile } from "@/shared/domain/legacy-mission";
import type { TaskStatus } from "@/features/task";
import type { InferredAgentStatus } from "../state/screen-types.js";

// ── Palette (256-color) ─────────────────────────────

export const PALETTE = {
  // Greens
  green: 34,
  brightGreen: 46,
  // Yellows
  yellow: 220,
  amber: 214,
  // Reds
  red: 196,
  // Blues
  blue: 33,
  cyan: 39,
  // Grays
  gray: 245,
  dimGray: 240,
  brightWhite: 255,
  border: 240,
  // Accents
  magenta: 200,
  orange: 208,
  // Background
  panelBg: 235,
  infoBg: 237,
  selectedBg: 236,
  headerBg: 234,
    overlayBackdropBg: -1,
    overlaySurfaceBg: -1,
    overlayQueryBg: -1,
    overlaySection: 250,
    overlayHint: 254,
    overlaySelectedBg: 39,
    overlaySelectedFg: 255,
    progressUnfilledBg: 238,
    // Default (terminal default)
    default: -1,
} as const;

// ── Status Colors ───────────────────────────────────

export const MISSION_STATUS_COLOR: Record<MissionStatus, number> = {
  draft: PALETTE.gray,
  approved: PALETTE.blue,
  rejected: PALETTE.red,
  executing: PALETTE.orange,
  paused: PALETTE.amber,
  validating: PALETTE.yellow,
  completed: PALETTE.brightGreen,
  failed: PALETTE.red,
};

export const FEATURE_STATUS_COLOR: Record<FeatureStatus, number> = {
  pending: PALETTE.dimGray,
  assigned: PALETTE.yellow,
  "in-progress": PALETTE.green,
  review: PALETTE.cyan,
  done: PALETTE.green,
  blocked: PALETTE.red,
};

export const MILESTONE_STATUS_COLOR: Record<MilestoneStatus, number> = {
  pending: PALETTE.dimGray,
  executing: PALETTE.green,
  validating: PALETTE.yellow,
  sealed: PALETTE.brightGreen,
  failed: PALETTE.red,
};

export const ASSERTION_RESULT_COLOR: Record<AssertionResult, number> = {
  pending: PALETTE.dimGray,
  passed: PALETTE.green,
  failed: PALETTE.red,
  blocked: PALETTE.amber,
  waived: PALETTE.gray,
};

// ── Status Dots ─────────────────────────────────────

export const DOT_FILLED = "\u25cf";  // ●
export const DOT_EMPTY = "\u25cb";   // ○

/** Get the dot character for a feature status. */
export function featureDot(status: FeatureStatus): string {
  switch (status) {
    case "done":
    case "review":
    case "in-progress":
    case "assigned":
      return DOT_FILLED;
    case "blocked":
      return "x";
    default:
      return DOT_EMPTY;
  }
}

// ── Status Labels ───────────────────────────────────

export const MISSION_STATUS_LABEL: Record<MissionStatus, string> = {
  draft: "DRAFT",
  approved: "APPROVED",
  rejected: "REJECTED",
  executing: "RUNNING",
  paused: "PAUSED",
  validating: "VALIDATING",
  completed: "COMPLETED",
  failed: "FAILED",
};

export const FEATURE_STATUS_LABEL: Record<FeatureStatus, string> = {
  pending: "pending",
  assigned: "assigned",
  "in-progress": "in-prog",
  review: "review",
  done: "done",
  blocked: "blocked",
};

export const FEATURE_TASK_STATUS_LABEL: Record<FeatureStatus, string> = {
  pending: "OPEN",
  assigned: "OPEN",
  "in-progress": "OPEN",
  review: "REVIEW",
  done: "DONE",
  blocked: "BLOCKED",
};

export const TASK_STATUS_COLUMN_LABEL: Record<TaskStatus, string> = {
  pending: "Pending",
  in_progress: "In Progress",
  completed: "Completed",
};

export const AGENT_STATUS_LABEL: Record<InferredAgentStatus, string> = {
  active: "Active",
  waiting: "Waiting",
  completed: "Done",
  stale: "Stale",
};

export const FEATURE_TASK_STATUS_COLOR: Record<FeatureStatus, number> = {
  pending: PALETTE.cyan,
  assigned: PALETTE.cyan,
  "in-progress": PALETTE.cyan,
  review: PALETTE.yellow,
  done: PALETTE.green,
  blocked: PALETTE.red,
};

// ── Milestone Kind / Profile ───────────────────────

export const MILESTONE_KIND_INDICATOR: Record<MilestoneKind, string> = {
  work: ">>",
  gate: "||",
};

export const MILESTONE_KIND_COLOR: Record<MilestoneKind, number> = {
  work: PALETTE.green,
  gate: PALETTE.amber,
};

export const MILESTONE_PROFILE_LABEL: Record<MilestoneProfile, string> = {
  planning: "PLAN",
  "plan-review": "PLAN-REVIEW",
  implementation: "IMPL",
  "code-review": "CODE-REVIEW",
  "bug-hunt": "BUG-HUNT",
  simplify: "SIMPLIFY",
  validation: "VALIDATE",
  custom: "CUSTOM",
};

export const MILESTONE_PROFILE_COLOR: Record<MilestoneProfile, number> = {
  planning: PALETTE.cyan,
  "plan-review": PALETTE.amber,
  implementation: PALETTE.green,
  "code-review": PALETTE.amber,
  "bug-hunt": PALETTE.red,
  simplify: PALETTE.cyan,
  validation: PALETTE.yellow,
  custom: PALETTE.gray,
};
