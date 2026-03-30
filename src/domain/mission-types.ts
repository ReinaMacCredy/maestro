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
  | "paused"
  | "validating"
  | "completed"
  | "failed";

/** Milestone lifecycle status */
export type MilestoneStatus =
  | "pending"
  | "executing"
  | "validating"
  | "sealed"
  | "failed";

/** Feature lifecycle status */
export type FeatureStatus =
  | "pending"
  | "assigned"
  | "in-progress"
  | "review"
  | "done"
  | "blocked";

/** Assertion validation result status - includes 'waived' as terminal state */
export type AssertionResult =
  | "pending"
  | "passed"
  | "failed"
  | "blocked"
  | "waived";

/** Test surface classification for assertions */
export type AssertionSurface = "browser" | "cli" | "api" | (string & {});

// ============================
// Worker Report Sub-types
// ============================

/** Record of a command executed during worker verification */
export interface CommandRun {
  readonly command: string;
  readonly exitCode: number;
  readonly observation: string;
}

/** Record of an interactive check performed by a worker */
export interface InteractiveCheck {
  readonly action: string;
  readonly observed: string;
}

/** A single test case within a test file */
export interface TestCase {
  readonly name: string;
  readonly verifies: string;
}

/** A test file added by a worker */
export interface TestFile {
  readonly file: string;
  readonly cases: readonly TestCase[];
}

/** An issue discovered during worker execution */
export interface DiscoveredIssue {
  readonly severity: string;
  readonly description: string;
  readonly suggestedFix?: string;
}

// ============================
// Core Entity Types
// ============================

/** Milestone definition within a mission */
export interface Milestone {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly order: number;
  readonly featureIds: readonly string[];
}

/** Milestone input used before mission state is materialized */
export interface MilestoneInput {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly order: number;
}

/** Structured worker report attached to a feature */
export interface WorkerReport {
  readonly salientSummary: string;
  readonly whatWasImplemented: string;
  readonly whatWasLeftUndone: string;
  readonly verification: {
    readonly commandsRun: readonly CommandRun[];
    readonly interactiveChecks: readonly InteractiveCheck[];
  };
  readonly tests: {
    readonly added: readonly TestFile[];
  };
  readonly discoveredIssues: readonly DiscoveredIssue[];
}

/** Feature within a mission */
export interface Feature {
  readonly id: string;
  readonly missionId: string;
  readonly milestoneId: string;
  readonly status: FeatureStatus;
  readonly title: string;
  readonly description: string;
  readonly workerType: string;
  readonly verificationSteps: readonly string[];
  readonly dependsOn: readonly string[];
  readonly fulfills: readonly string[];
  readonly preconditions?: string;
  readonly expectedBehavior?: string;
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
  readonly result: AssertionResult;
  readonly description: string;
  readonly surface: AssertionSurface;
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
  readonly proposal?: string;
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
  readonly currentMilestoneId: string;
  readonly timestamp: string;
  readonly featureStatuses: Readonly<Record<string, FeatureStatus>>;
  readonly assertionResults: Readonly<Record<string, AssertionResult>>;
}

// ============================
// Create/Update Input Types
// ============================

/** Input for creating a new mission */
export interface CreateMissionInput {
  readonly title: string;
  readonly description: string;
  readonly proposal?: string;
  readonly milestones: readonly MilestoneInput[];
}

/** Input for creating a new feature */
export interface CreateFeatureInput {
  readonly missionId: string;
  readonly milestoneId: string;
  readonly title: string;
  readonly description: string;
  readonly workerType: string;
  readonly verificationSteps: readonly string[];
  readonly dependsOn?: readonly string[];
  readonly fulfills?: readonly string[];
  readonly preconditions?: string;
  readonly expectedBehavior?: string;
}

/** Feature definition from a mission plan file */
export interface MissionPlanFeature {
  readonly id: string;
  readonly milestoneId: string;
  readonly title: string;
  readonly description: string;
  readonly workerType: string;
  readonly verificationSteps: readonly string[];
  readonly dependsOn?: readonly string[];
  readonly fulfills?: readonly string[];
  readonly preconditions?: string;
  readonly expectedBehavior?: string;
}

/** Mission plan file as accepted by `mission create` */
export interface MissionPlanFile {
  readonly title: string;
  readonly description?: string;
  readonly proposal?: string;
  readonly milestones: readonly MilestoneInput[];
  readonly features: readonly MissionPlanFeature[];
}

/** Input for creating a new assertion */
export interface CreateAssertionInput {
  readonly missionId: string;
  readonly milestoneId: string;
  readonly featureId: string;
  readonly description: string;
  readonly surface?: AssertionSurface;
}

/** Input for updating an assertion */
export interface UpdateAssertionInput {
  readonly result: AssertionResult;
  readonly evidence?: string;
  readonly waivedReason?: string;
}

/** Input for updating a feature */
export interface UpdateFeatureInput {
  readonly status?: FeatureStatus;
  readonly report?: WorkerReport;
  readonly retryReason?: string;
}

/** Input for updating a mission */
export interface UpdateMissionInput {
  readonly status?: MissionStatus;
  readonly title?: string;
  readonly description?: string;
  readonly completedMilestoneIds?: readonly string[];
}
