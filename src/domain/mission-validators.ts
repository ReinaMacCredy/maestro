/**
 * Mission Control domain validators
 * Zod schemas and validation functions for mission entities
 */

import { z } from "zod";
import { MaestroError } from "./errors.js";
import type {
  Mission,
  Milestone,
  MilestoneInput,
  Feature,
  Assertion,
  Checkpoint,
  CreateMissionInput,
  CreateFeatureInput,
  CreateAssertionInput,
  UpdateAssertionInput,
  MissionPlanFile,
} from "./mission-types.js";
import type { WorkflowTemplate } from "./types.js";

// ============================
// Schema Constants
// ============================

const MISSION_ID_PATTERN = /^\d{4}-\d{2}-\d{2}-\d{3}$/;
const ISO_DATE_PATTERN = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{3})?Z$/;
export const FEATURE_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_-]*$/;
export const WORKER_TYPE_PATTERN = /^[A-Za-z0-9][A-Za-z0-9:_-]*$/;

// ============================
// Zod Schemas
// ============================

export const MilestoneKindSchema = z.enum(["work", "gate"]);

export const MilestoneProfileSchema = z.enum([
  "planning",
  "plan-review",
  "implementation",
  "code-review",
  "bug-hunt",
  "simplify",
  "validation",
  "custom",
]);

export const WorkflowPhaseSchema = z.object({
  kind: MilestoneKindSchema,
  label: z.string().min(1),
  profile: MilestoneProfileSchema.optional(),
  description: z.string().optional(),
}).strict();

export const WorkflowTemplateSchema = z.object({
  description: z.string().min(1),
  phases: z.array(WorkflowPhaseSchema).min(1),
}).strict();

export const MilestoneInputSchema = z.object({
  id: z.string().min(1),
  title: z.string().min(1),
  description: z.string(),
  order: z.number().int().nonnegative(),
  kind: MilestoneKindSchema.default("work").optional(),
  profile: MilestoneProfileSchema.default("custom").optional(),
}).strict();

export const MilestoneSchema = MilestoneInputSchema.extend({
  featureIds: z.array(z.string().min(1)).default([]),
}).strict();

const CommandRunSchema = z.object({
  command: z.string().min(1),
  exitCode: z.number().int(),
  observation: z.string(),
}).strict();

const InteractiveCheckSchema = z.object({
  action: z.string().min(1),
  observed: z.string(),
}).strict();

const TestCaseSchema = z.object({
  name: z.string().min(1),
  verifies: z.string(),
}).strict();

const TestFileSchema = z.object({
  file: z.string().min(1),
  cases: z.array(TestCaseSchema),
}).strict();

const DiscoveredIssueSchema = z.object({
  severity: z.string().min(1),
  description: z.string().min(1),
  suggestedFix: z.string().optional(),
}).strict();

/** Rich worker report (plan spec) */
const RichWorkerReportSchema = z.object({
  salientSummary: z.string().min(1),
  whatWasImplemented: z.string(),
  whatWasLeftUndone: z.string(),
  verification: z.object({
    commandsRun: z.array(CommandRunSchema),
    interactiveChecks: z.array(InteractiveCheckSchema),
  }).strict(),
  tests: z.object({
    added: z.array(TestFileSchema),
  }).strict(),
  discoveredIssues: z.array(DiscoveredIssueSchema),
}).strict();

/** Legacy worker report (backward compat -- transforms to rich format) */
const LegacyWorkerReportSchema = z.object({
  content: z.string().min(1),
  timestamp: z.string().optional(),
  agent: z.string().optional(),
}).strict().transform((legacy) => ({
  salientSummary: legacy.content,
  whatWasImplemented: legacy.content,
  whatWasLeftUndone: "",
  verification: { commandsRun: [] as readonly z.infer<typeof CommandRunSchema>[], interactiveChecks: [] as readonly z.infer<typeof InteractiveCheckSchema>[] },
  tests: { added: [] as readonly z.infer<typeof TestFileSchema>[] },
  discoveredIssues: [] as readonly z.infer<typeof DiscoveredIssueSchema>[],
}));

/** Accepts rich or legacy worker report, normalizes to rich format */
export const WorkerReportSchema = z.union([RichWorkerReportSchema, LegacyWorkerReportSchema]);

export const FeatureSchema = z.object({
  id: z.string().regex(FEATURE_ID_PATTERN),
  missionId: z.string().regex(MISSION_ID_PATTERN),
  milestoneId: z.string().min(1),
  status: z.enum(["pending", "assigned", "in-progress", "review", "done", "blocked"]),
  title: z.string().min(1),
  description: z.string(),
  workerType: z.string().regex(WORKER_TYPE_PATTERN),
  verificationSteps: z.array(z.string().min(1)).min(1),
  dependsOn: z.array(z.string()).default([]),
  fulfills: z.array(z.string()).default([]),
  preconditions: z.string().optional(),
  expectedBehavior: z.string().optional(),
  report: WorkerReportSchema.optional(),
  createdAt: z.string().regex(ISO_DATE_PATTERN),
  updatedAt: z.string().regex(ISO_DATE_PATTERN),
}).strict();

export const AssertionSchema = z.object({
  id: z.string().min(1),
  missionId: z.string().regex(MISSION_ID_PATTERN),
  milestoneId: z.string().min(1),
  featureId: z.string().min(1),
  result: z.enum(["pending", "passed", "failed", "blocked", "waived"]),
  description: z.string().min(1),
  surface: z.string().default("cli"),
  evidence: z.string().optional(),
  waivedReason: z.string().min(1).optional(),
  createdAt: z.string().regex(ISO_DATE_PATTERN),
  updatedAt: z.string().regex(ISO_DATE_PATTERN),
}).strict().refine(
  (data) => {
    // If result is waived, waivedReason must be provided and non-empty
    if (data.result === "waived") {
      return data.waivedReason !== undefined && data.waivedReason.length > 0;
    }
    return true;
  },
  {
    message: "waivedReason is required when result is 'waived'",
    path: ["waivedReason"],
  },
);

export const MissionSchema = z.object({
  id: z.string().regex(MISSION_ID_PATTERN),
  status: z.enum(["draft", "approved", "rejected", "executing", "paused", "validating", "completed", "failed"]),
  title: z.string().min(1),
  description: z.string(),
  proposal: z.string().optional(),
  milestones: z.array(MilestoneSchema),
  features: z.array(z.string().min(1)),
  createdAt: z.string().regex(ISO_DATE_PATTERN),
  updatedAt: z.string().regex(ISO_DATE_PATTERN),
  approvedAt: z.string().regex(ISO_DATE_PATTERN).optional(),
  rejectedAt: z.string().regex(ISO_DATE_PATTERN).optional(),
  completedAt: z.string().regex(ISO_DATE_PATTERN).optional(),
  completedMilestoneIds: z.array(z.string().min(1)).optional(),
}).strict().refine(
  (data) => {
    // Validate that milestone IDs are unique
    const milestoneIds = data.milestones.map((m) => m.id);
    return new Set(milestoneIds).size === milestoneIds.length;
  },
  {
    message: "Milestone IDs must be unique",
    path: ["milestones"],
  },
);

export const CheckpointSchema = z.object({
  id: z.string().min(1),
  missionId: z.string().regex(MISSION_ID_PATTERN),
  currentMilestoneId: z.string().min(1),
  timestamp: z.string().regex(ISO_DATE_PATTERN),
  featureStatuses: z.record(z.enum(["pending", "assigned", "in-progress", "review", "done", "blocked"])),
  assertionResults: z.record(z.enum(["pending", "passed", "failed", "blocked", "waived"])),
}).strict();

// Input validation schemas
export const CreateMissionInputSchema = z.object({
  title: z.string().min(1),
  description: z.string(),
  proposal: z.string().optional(),
  milestones: z.array(MilestoneInputSchema).min(1),
}).strict().refine(
  (data) => {
    const milestoneIds = data.milestones.map((m) => m.id);
    return new Set(milestoneIds).size === milestoneIds.length;
  },
  {
    message: "Milestone IDs must be unique",
    path: ["milestones"],
  },
);

export const CreateFeatureInputSchema = z.object({
  missionId: z.string().regex(MISSION_ID_PATTERN),
  milestoneId: z.string().min(1),
  title: z.string().min(1),
  description: z.string(),
  workerType: z.string().regex(WORKER_TYPE_PATTERN),
  verificationSteps: z.array(z.string().min(1)).min(1),
  dependsOn: z.array(z.string()).optional(),
  fulfills: z.array(z.string()).optional(),
  preconditions: z.string().optional(),
  expectedBehavior: z.string().optional(),
}).strict();

export const CreateAssertionInputSchema = z.object({
  missionId: z.string().regex(MISSION_ID_PATTERN),
  milestoneId: z.string().min(1),
  featureId: z.string().regex(FEATURE_ID_PATTERN),
  description: z.string().min(1),
  surface: z.string().default("cli"),
}).strict();

const MissionPlanFeatureSchema = z.object({
  id: z.string().regex(FEATURE_ID_PATTERN),
  milestoneId: z.string().min(1),
  title: z.string().min(1),
  description: z.string(),
  workerType: z.string().regex(WORKER_TYPE_PATTERN),
  verificationSteps: z.array(z.string().min(1)).min(1),
  dependsOn: z.array(z.string()).optional(),
  fulfills: z.array(z.string()).optional(),
  preconditions: z.string().optional(),
  expectedBehavior: z.string().optional(),
}).strict();

export const MissionPlanFileSchema = z.object({
  title: z.string().min(1),
  description: z.string().optional(),
  proposal: z.string().optional(),
  milestones: z.array(MilestoneInputSchema).min(1),
  features: z.array(MissionPlanFeatureSchema),
}).strict();

export const UpdateAssertionInputSchema = z.object({
  result: z.enum(["pending", "passed", "failed", "blocked", "waived"]),
  evidence: z.string().optional(),
  waivedReason: z.string().min(1).optional(),
}).strict().refine(
  (data) => {
    if (data.result === "waived") {
      return data.waivedReason !== undefined && data.waivedReason.length > 0;
    }
    return true;
  },
  {
    message: "waivedReason is required when result is 'waived'",
    path: ["waivedReason"],
  },
);

// ============================
// Validation Functions
// ============================

export function validateMission(data: unknown): Mission {
  return MissionSchema.parse(data);
}

export function validateMilestone(data: unknown): Milestone {
  return MilestoneSchema.parse(data);
}

export function validateFeature(data: unknown): Feature {
  return FeatureSchema.parse(data);
}

export function validateAssertion(data: unknown): Assertion {
  return AssertionSchema.parse(data);
}

export function validateCheckpoint(data: unknown): Checkpoint {
  return CheckpointSchema.parse(data);
}

export function validateCreateMissionInput(data: unknown): CreateMissionInput {
  return CreateMissionInputSchema.parse(data);
}

export function validateMissionPlanFile(data: unknown): MissionPlanFile {
  try {
    return MissionPlanFileSchema.parse(data);
  } catch (err) {
    if (err instanceof z.ZodError) {
      const issue = err.issues[0];
      const path = issue?.path.length ? issue.path.join(".") : "root";
      throw new MaestroError(`Invalid mission plan file: ${issue?.message ?? "validation failed"}`, [
        `Problem field: ${path}`,
        "Mission plans must include title, milestones, and a features array",
      ]);
    }
    throw err;
  }
}

export function validateWorkflowTemplate(data: unknown, templateName: string): WorkflowTemplate {
  try {
    return WorkflowTemplateSchema.parse(data);
  } catch (err) {
    if (err instanceof z.ZodError) {
      const issue = err.issues[0];
      const path = issue?.path.length ? issue.path.join(".") : "root";
      throw new MaestroError(
        `Invalid workflow template '${templateName}': ${issue?.message ?? "validation failed"}`,
        [
          `Problem field: ${path}`,
          "Workflow templates must include a description and at least one phase with kind and label",
        ],
      );
    }
    throw err;
  }
}

export function validateCreateFeatureInput(data: unknown): CreateFeatureInput {
  return CreateFeatureInputSchema.parse(data);
}

export function validateCreateAssertionInput(data: unknown): CreateAssertionInput {
  return CreateAssertionInputSchema.parse(data);
}

export function validateUpdateAssertionInput(data: unknown): UpdateAssertionInput {
  return UpdateAssertionInputSchema.parse(data);
}

// ============================
// Referential Integrity Validation
// ============================

/**
 * Assert that no dangling references exist in mission data.
 * Checks:
 * - All feature milestoneIds reference existing milestones
 * - All mission feature IDs exist in features array
 * - All assertion featureIds and milestoneIds reference existing entities
 */
export function assertNoDanglingReferences(
  mission: Mission,
  features: readonly Feature[],
  assertions: readonly Assertion[],
): void {
  const milestoneIds = new Set(mission.milestones.map((m) => m.id));
  const featureIds = new Set(features.map((f) => f.id));
  const missionFeatureIds = new Set(mission.features);

  // Check all features reference valid milestones
  for (const feature of features) {
    if (!milestoneIds.has(feature.milestoneId)) {
      throw new MaestroError(
        `Dangling reference: Feature '${feature.id}' references non-existent milestone '${feature.milestoneId}'`,
        [
          `Available milestones: ${Array.from(milestoneIds).join(", ")}`,
          `Check the milestoneId in feature '${feature.id}'`,
        ],
      );
    }
  }

  // Check all mission feature IDs exist
  for (const featureId of mission.features) {
    if (!featureIds.has(featureId)) {
      throw new MaestroError(
        `Dangling reference: Mission features list contains non-existent feature '${featureId}'`,
        [
          `Available features: ${Array.from(featureIds).join(", ")}`,
          `Remove '${featureId}' from mission.features or create the feature`,
        ],
      );
    }
  }

  // Check all assertions reference valid features and milestones
  for (const assertion of assertions) {
    if (!featureIds.has(assertion.featureId)) {
      throw new MaestroError(
        `Dangling reference: Assertion '${assertion.id}' references non-existent feature '${assertion.featureId}'`,
        [
          `Available features: ${Array.from(featureIds).join(", ")}`,
          `Check the featureId in assertion '${assertion.id}'`,
        ],
      );
    }
    if (!milestoneIds.has(assertion.milestoneId)) {
      throw new MaestroError(
        `Dangling reference: Assertion '${assertion.id}' references non-existent milestone '${assertion.milestoneId}'`,
        [
          `Available milestones: ${Array.from(milestoneIds).join(", ")}`,
          `Check the milestoneId in assertion '${assertion.id}'`,
        ],
      );
    }
  }
}

// ============================
// Cyclic Dependency Validation
// ============================

/**
 * Assert that no cyclic dependencies exist in feature dependency graph.
 * Uses DFS to detect cycles in the directed graph formed by feature dependsOn arrays.
 */
export function assertNoCyclicDependencies(features: readonly Feature[]): void {
  const featureMap = new Map(features.map((f) => [f.id, f]));
  const visiting = new Set<string>();
  const visited = new Set<string>();

  function visit(featureId: string, path: string[]): void {
    if (visiting.has(featureId)) {
      // Found a cycle
      const cycleStart = path.indexOf(featureId);
      const cycle = path.slice(cycleStart).concat(featureId);
      throw new MaestroError(
        `Cyclic dependency detected: ${cycle.join(" -> ")}`,
        [
          `Review the 'dependsOn' arrays for features in this cycle`,
          `Remove circular references to fix the dependency graph`,
        ],
      );
    }

    if (visited.has(featureId)) {
      return;
    }

    const feature = featureMap.get(featureId);
    if (!feature) {
      throw new MaestroError(
        `Dangling dependency: Feature '${path[path.length - 1]}' depends on non-existent feature '${featureId}'`,
        [
          `Available features: ${Array.from(featureMap.keys()).join(", ")}`,
          `Update the 'dependsOn' array to reference only existing features`,
        ],
      );
    }

    visiting.add(featureId);
    path.push(featureId);

    for (const depId of feature.dependsOn) {
      visit(depId, [...path]);
    }

    visiting.delete(featureId);
    visited.add(featureId);
  }

  for (const feature of features) {
    if (!visited.has(feature.id)) {
      visit(feature.id, []);
    }
  }
}
