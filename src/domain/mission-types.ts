/**
 * Mission Control domain types
 * Defines core entities: Mission, Milestone, Feature, Assertion, Checkpoint
 */

// ============================
// Status Types
// ============================

/** Mission lifecycle status */
export type MissionStatus =
  | "draft"
  | "approved"
  | "rejected"
  | "executing"
  | "validating"
  | "completed"
  | "failed";

/** Milestone lifecycle status */
export type MilestoneStatus =
  | "pending"
  | "executing"
  | "validating"
  | "completed"
  | "failed";

/** Feature lifecycle status */
export type FeatureStatus =
  | "pending"
  | "in_progress"
  | "in_review"
  | "completed"
  | "blocked";

/** Assertion validation result status - includes 'waived' as terminal state */
export type AssertionStatus =
  | "pending"
  | "passed"
  | "failed"
  | "blocked"
  | "waived";

// ============================
// Core Entity Types
// ============================

/** Milestone definition within a mission */
export interface Milestone {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly order: number;
}

/** Worker report attached to a feature */
export interface WorkerReport {
  readonly content: string;
  readonly timestamp: string;
  readonly agent?: string;
}

/** Feature within a mission */
export interface Feature {
  readonly id: string;
  readonly missionId: string;
  readonly milestoneId: string;
  readonly status: FeatureStatus;
  readonly title: string;
  readonly description: string;
  readonly skillName: string;
  readonly verificationSteps: readonly string[];
  readonly dependsOn: readonly string[];
  readonly report?: WorkerReport;
  readonly createdAt: string;
  readonly updatedAt: string;
}

/** Assertion for validating feature implementation */
export interface Assertion {
  readonly id: string;
  readonly missionId: string;
  readonly milestoneId: string;
  readonly featureId: string;
  readonly status: AssertionStatus;
  readonly description: string;
  readonly evidence?: string;
  readonly waivedReason?: string;
  readonly createdAt: string;
  readonly updatedAt: string;
}

/** Mission - top-level plan container */
export interface Mission {
  readonly id: string;
  readonly status: MissionStatus;
  readonly title: string;
  readonly description: string;
  readonly milestones: readonly Milestone[];
  readonly features: readonly string[]; // Feature IDs
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly approvedAt?: string;
  readonly rejectedAt?: string;
  readonly completedAt?: string;
  readonly completedMilestoneIds?: readonly string[];
}

/** Checkpoint - saved state snapshot */
export interface Checkpoint {
  readonly id: string;
  readonly missionId: string;
  readonly milestoneId: string;
  readonly timestamp: string;
  readonly featureStates: Readonly<Record<string, FeatureStatus>>;
  readonly assertionStates: Readonly<Record<string, AssertionStatus>>;
}

// ============================
// Create/Update Input Types
// ============================

/** Input for creating a new mission */
export interface CreateMissionInput {
  readonly title: string;
  readonly description: string;
  readonly milestones: readonly Milestone[];
}

/** Input for creating a new feature */
export interface CreateFeatureInput {
  readonly missionId: string;
  readonly milestoneId: string;
  readonly title: string;
  readonly description: string;
  readonly skillName: string;
  readonly verificationSteps: readonly string[];
  readonly dependsOn?: readonly string[];
}

/** Input for creating a new assertion */
export interface CreateAssertionInput {
  readonly missionId: string;
  readonly milestoneId: string;
  readonly featureId: string;
  readonly description: string;
}

/** Input for updating an assertion */
export interface UpdateAssertionInput {
  readonly status: AssertionStatus;
  readonly evidence?: string;
  readonly waivedReason?: string;
}

/** Input for updating a feature */
export interface UpdateFeatureInput {
  readonly status?: FeatureStatus;
  readonly report?: WorkerReport;
}

/** Input for updating a mission */
export interface UpdateMissionInput {
  readonly status?: MissionStatus;
  readonly title?: string;
  readonly description?: string;
  readonly completedMilestoneIds?: readonly string[];
}
