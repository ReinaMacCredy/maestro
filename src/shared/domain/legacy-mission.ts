export type MissionStatus =
  | "draft"
  | "approved"
  | "rejected"
  | "executing"
  | "paused"
  | "validating"
  | "completed"
  | "failed";

export type FeatureStatus =
  | "pending"
  | "assigned"
  | "in-progress"
  | "review"
  | "done"
  | "blocked";

export type MilestoneStatus = "pending" | "executing" | "validating" | "sealed" | "failed";
export type AssertionResult = "pending" | "passed" | "failed" | "blocked" | "waived";
export type MilestoneKind = "work" | "gate";
export type MilestoneProfile =
  | "planning"
  | "plan-review"
  | "implementation"
  | "code-review"
  | "bug-hunt"
  | "simplify"
  | "validation"
  | "custom";

export interface Mission {
  readonly id: string;
  readonly title: string;
  readonly status: MissionStatus;
}

export interface Feature {
  readonly id: string;
  readonly title: string;
  readonly status: FeatureStatus;
  readonly dependsOn: readonly string[];
}

export interface Milestone {
  readonly id: string;
  readonly title: string;
  readonly status: MilestoneStatus;
  readonly order: number;
  readonly kind?: MilestoneKind;
  readonly profile?: MilestoneProfile;
}

export interface Checkpoint {
  readonly id: string;
  readonly title: string;
  readonly createdAt?: string;
}

export interface Assertion {
  readonly id: string;
  readonly title: string;
  readonly result: AssertionResult;
  readonly createdAt?: string;
}

const TRANSITIONS: Readonly<Record<FeatureStatus, readonly FeatureStatus[]>> = {
  pending: ["assigned", "blocked", "done"],
  assigned: ["in-progress", "blocked", "done"],
  "in-progress": ["review", "blocked", "done"],
  review: ["done", "in-progress", "blocked"],
  done: [],
  blocked: ["pending", "assigned"],
};

export function getValidFeatureTransitions(status: FeatureStatus): readonly FeatureStatus[] {
  return TRANSITIONS[status] ?? [];
}

export async function updateFeature(
  _missionStore: unknown,
  _featureStore: unknown,
  _cwd: string,
  _missionId: string,
  _featureId: string,
  _patch: Readonly<Record<string, unknown>>,
): Promise<void> {
  throw new Error("Mission Control is read-only in the restored TypeScript sidecar");
}
