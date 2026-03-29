/**
 * Mission Control state machine transitions
 * Defines valid transitions for Mission, Milestone, Feature, and Assertion states
 */

import { MaestroError } from "./errors.js";
import type {
  MissionStatus,
  MilestoneStatus,
  FeatureStatus,
  AssertionStatus,
} from "./mission-types.js";

// ============================
// Mission State Machine
// ============================

const MISSION_TRANSITIONS: Readonly<Record<MissionStatus, readonly MissionStatus[]>> = {
  draft: ["approved", "rejected"],
  approved: ["executing"],
  rejected: [], // terminal
  executing: ["validating"],
  validating: ["completed", "failed"],
  completed: [], // terminal
  failed: [], // terminal
};

/** Check if a mission transition is valid */
export function canTransitionMission(
  from: MissionStatus,
  to: MissionStatus,
): boolean {
  return MISSION_TRANSITIONS[from].includes(to);
}

/** Get list of valid next states for a mission */
export function getValidMissionTransitions(from: MissionStatus): readonly MissionStatus[] {
  return MISSION_TRANSITIONS[from];
}

/** Assert that a mission transition is valid, throw MaestroError if not */
export function assertMissionTransition(
  from: MissionStatus,
  to: MissionStatus,
): void {
  if (!canTransitionMission(from, to)) {
    const validNext = getValidMissionTransitions(from);
    const hint = validNext.length > 0
      ? `Valid transitions from ${from}: ${validNext.join(", ")}`
      : `${from} is a terminal state - no transitions allowed`;
    throw new MaestroError(
      `Invalid mission transition: ${from} -> ${to}`,
      [hint, `Use 'maestro mission show' to view current state`],
    );
  }
}

// ============================
// Milestone State Machine
// ============================

const MILESTONE_TRANSITIONS: Readonly<Record<MilestoneStatus, readonly MilestoneStatus[]>> = {
  pending: ["executing"],
  executing: ["validating"],
  validating: ["completed", "failed"],
  completed: [], // terminal
  failed: [], // terminal
};

/** Check if a milestone transition is valid */
export function canTransitionMilestone(
  from: MilestoneStatus,
  to: MilestoneStatus,
): boolean {
  return MILESTONE_TRANSITIONS[from].includes(to);
}

/** Get list of valid next states for a milestone */
export function getValidMilestoneTransitions(from: MilestoneStatus): readonly MilestoneStatus[] {
  return MILESTONE_TRANSITIONS[from];
}

/** Assert that a milestone transition is valid, throw MaestroError if not */
export function assertMilestoneTransition(
  from: MilestoneStatus,
  to: MilestoneStatus,
): void {
  if (!canTransitionMilestone(from, to)) {
    const validNext = getValidMilestoneTransitions(from);
    const hint = validNext.length > 0
      ? `Valid transitions from ${from}: ${validNext.join(", ")}`
      : `${from} is a terminal state - no transitions allowed`;
    throw new MaestroError(
      `Invalid milestone transition: ${from} -> ${to}`,
      [hint, `Use 'maestro milestone status' to view current state`],
    );
  }
}

// ============================
// Feature State Machine
// ============================

const FEATURE_TRANSITIONS: Readonly<Record<FeatureStatus, readonly FeatureStatus[]>> = {
  pending: ["in_progress"],
  in_progress: ["in_review"],
  in_review: ["completed", "blocked", "pending"], // pending = retry
  completed: [], // terminal
  blocked: ["pending"], // pending = retry
};

/** Check if a feature transition is valid */
export function canTransitionFeature(
  from: FeatureStatus,
  to: FeatureStatus,
): boolean {
  return FEATURE_TRANSITIONS[from].includes(to);
}

/** Get list of valid next states for a feature */
export function getValidFeatureTransitions(from: FeatureStatus): readonly FeatureStatus[] {
  return FEATURE_TRANSITIONS[from];
}

/** Assert that a feature transition is valid, throw MaestroError if not */
export function assertFeatureTransition(
  from: FeatureStatus,
  to: FeatureStatus,
): void {
  if (!canTransitionFeature(from, to)) {
    const validNext = getValidFeatureTransitions(from);
    const hint = validNext.length > 0
      ? `Valid transitions from ${from}: ${validNext.join(", ")}`
      : `${from} is a terminal state - no transitions allowed`;
    throw new MaestroError(
      `Invalid feature transition: ${from} -> ${to}`,
      [hint, `Use 'maestro feature list' to view current state`],
    );
  }
}

// ============================
// Assertion State Machine
// ============================

const ASSERTION_TRANSITIONS: Readonly<Record<AssertionStatus, readonly AssertionStatus[]>> = {
  pending: ["passed", "failed", "blocked", "waived"],
  passed: [], // terminal
  failed: ["pending"], // retry
  blocked: ["pending"], // retry
  waived: [], // terminal - preserved as terminal per requirements
};

/** Check if an assertion transition is valid */
export function canTransitionAssertion(
  from: AssertionStatus,
  to: AssertionStatus,
): boolean {
  return ASSERTION_TRANSITIONS[from].includes(to);
}

/** Get list of valid next states for an assertion */
export function getValidAssertionTransitions(from: AssertionStatus): readonly AssertionStatus[] {
  return ASSERTION_TRANSITIONS[from];
}

/** Assert that an assertion transition is valid, throw MaestroError if not */
export function assertAssertionTransition(
  from: AssertionStatus,
  to: AssertionStatus,
): void {
  if (!canTransitionAssertion(from, to)) {
    const validNext = getValidAssertionTransitions(from);
    const hint = validNext.length > 0
      ? `Valid transitions from ${from}: ${validNext.join(", ")}`
      : `${from} is a terminal state - no transitions allowed`;
    throw new MaestroError(
      `Invalid assertion transition: ${from} -> ${to}`,
      [hint, `Use 'maestro validate show' to view current state`],
    );
  }
}

// ============================
// Terminal State Helpers
// ============================

/** Check if a mission status is terminal (no further transitions) */
export function isTerminalMissionStatus(status: MissionStatus): boolean {
  return MISSION_TRANSITIONS[status].length === 0;
}

/** Check if a milestone status is terminal */
export function isTerminalMilestoneStatus(status: MilestoneStatus): boolean {
  return MILESTONE_TRANSITIONS[status].length === 0;
}

/** Check if a feature status is terminal */
export function isTerminalFeatureStatus(status: FeatureStatus): boolean {
  return FEATURE_TRANSITIONS[status].length === 0;
}

/** Check if an assertion status is terminal (includes waived) */
export function isTerminalAssertionStatus(status: AssertionStatus): boolean {
  return ASSERTION_TRANSITIONS[status].length === 0;
}
