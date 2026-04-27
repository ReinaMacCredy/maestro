/**
 * Mission lifecycle usecases
 * Implements mission creation, approval, rejection, update, and listing
 */
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type { FeatureStorePort } from "../feature/ports/feature-store.port.js";
import type { AssertionStorePort } from "../ports/assertion-store.port.js";
import type {
  Mission,
  CreateMissionInput,
  UpdateMissionInput,
  CreateFeatureInput,
  CreateAssertionInput,
  MissionPlanFile,
  MilestoneInput,
  Feature,
} from "../domain/mission-types.js";
import { generateMissionId } from "../domain/mission-id.js";
import { MaestroError } from "@/shared/errors.js";
import type { MaestroConfig } from "@/infra/domain/config-types.js";
import type { WorkflowTemplate } from "../domain/workflow-types.js";
import { BUILT_IN_WORKFLOWS } from "../domain/workflows.js";
import {
  validateCreateMissionInput,
  validateMissionPlanFile,
  validateWorkflowTemplate,
  assertNoDanglingReferences,
  assertNoCyclicDependencies,
} from "../domain/mission-validators.js";
import { assertMissionTransition, canTransitionMission } from "../domain/mission-state.js";

/** Result of creating a mission */
export interface CreateMissionResult {
  mission: Mission;
  features: readonly Feature[];
}

/**
 * Expand a workflow template into a milestone array.
 * Resolves the template from config (user-defined) or built-in templates.
 * Generates milestone IDs from phase labels (kebab-cased) with order from array position.
 */
export function expandWorkflowTemplate(
  templateName: string,
  config?: MaestroConfig,
): readonly MilestoneInput[] {
  const userTemplates = config?.workflowTemplates ?? {};
  const rawTemplate =
    getOwnWorkflowTemplate(userTemplates, templateName)
    ?? getOwnWorkflowTemplate(BUILT_IN_WORKFLOWS, templateName);

  if (!rawTemplate) {
    const available = [
      ...Object.keys(BUILT_IN_WORKFLOWS),
      ...Object.keys(userTemplates),
    ];
    throw new MaestroError(`Unknown workflow template: ${templateName}`, [
      `Available templates: ${available.join(", ")}`,
    ]);
  }

  const template: WorkflowTemplate = validateWorkflowTemplate(rawTemplate, templateName);

  return template.phases.map((phase, index) => {
    const id = phase.label.toLowerCase().replace(/\s+/g, "-");
    return {
      id,
      title: phase.label,
      description: phase.description ?? phase.label,
      order: index,
      kind: phase.kind,
      profile: phase.profile,
    };
  });
}

function getOwnWorkflowTemplate(
  templates: Readonly<Record<string, WorkflowTemplate>>,
  templateName: string,
): WorkflowTemplate | undefined {
  if (!Object.hasOwn(templates, templateName)) {
    return undefined;
  }

  return templates[templateName];
}

/**
 * Create a new mission from a plan file
 * Validates cross-references, generates mission ID, creates all features and assertions
 */
export async function createMission(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
  planFile: MissionPlanFile,
): Promise<CreateMissionResult> {
  const validatedPlan = validateMissionPlanFile(planFile);

  // Validate input structure
  const input: CreateMissionInput = validateCreateMissionInput({
    title: validatedPlan.title,
    description: validatedPlan.description ?? "",
    proposal: validatedPlan.proposal,
    milestones: validatedPlan.milestones,
  });

  // Validate that all milestone IDs are unique
  const milestoneIds = new Set(input.milestones.map((m) => m.id));
  if (milestoneIds.size !== input.milestones.length) {
    throw new MaestroError("Duplicate milestone IDs found in plan", [
      "Each milestone must have a unique ID within the mission",
    ]);
  }

  // Validate that feature milestoneIds reference existing milestones
  for (const feature of validatedPlan.features) {
    if (!milestoneIds.has(feature.milestoneId)) {
      throw new MaestroError(
        `Feature '${feature.id}' references non-existent milestone '${feature.milestoneId}'`,
        [
          `Available milestones: ${Array.from(milestoneIds).join(", ")}`,
          `Check the milestoneId in feature '${feature.id}'`,
        ],
      );
    }
  }

  // Check for duplicate feature IDs
  const featureIds = new Set<string>();
  for (const feature of validatedPlan.features) {
    if (featureIds.has(feature.id)) {
      throw new MaestroError(`Duplicate feature ID: '${feature.id}'`, [
        "Feature IDs must be unique within a mission",
      ]);
    }
    featureIds.add(feature.id);
  }

  // Generate mission ID
  const existingIds = await missionStore.listIds();
  const missionId = generateMissionId(existingIds);

  // Stage the mission with features list (creates mission.json in staging area)
  await missionStore.stage(input, missionId, Array.from(featureIds));

  // Finalize the mission first (moves from staging, creates subdirectories)
  await missionStore.finalize(missionId);

  // Now create features and assertions in the final location
  // Wrap in try/catch for cleanup on failure (B8 fix)
  const features: Feature[] = [];
  try {
    for (const featureDef of validatedPlan.features) {
      const featureInput: CreateFeatureInput = {
        missionId,
        milestoneId: featureDef.milestoneId,
        title: featureDef.title,
        description: featureDef.description,
        agentType: featureDef.agentType,
        verificationSteps: featureDef.verificationSteps,
        dependsOn: featureDef.dependsOn ?? [],
        fulfills: featureDef.fulfills ?? [],
        preconditions: featureDef.preconditions,
        expectedBehavior: featureDef.expectedBehavior,
      };

      const feature = await featureStore.create(missionId, featureInput, featureDef.id);
      features.push(feature);

      // Create assertions for each fulfill
      if (featureDef.fulfills) {
        for (let i = 0; i < featureDef.fulfills.length; i++) {
          const assertionInput: CreateAssertionInput = {
            missionId,
            milestoneId: featureDef.milestoneId,
            featureId: featureDef.id,
            description: featureDef.fulfills[i]!,
          };
          await assertionStore.create(missionId, assertionInput, `${featureDef.id}-assertion-${i + 1}`);
        }
      }
    }

    // Validate no cyclic dependencies
    assertNoCyclicDependencies(features);

    // Validate referential integrity (B9 fix)
    const mission = await missionStore.get(missionId);
    if (!mission) {
      throw new MaestroError(`Failed to finalize mission ${missionId}`);
    }

    const allAssertions = await assertionStore.list(missionId);
    assertNoDanglingReferences(mission, features, allAssertions);

    return { mission, features };
  } catch (err) {
    // Attempt cleanup of partially created mission
    // The mission directory may be left in an inconsistent state -- wrap the
    // error with context so the user knows to clean up manually if needed.
    if (err instanceof MaestroError) throw err;
    throw new MaestroError(
      `Mission ${missionId} creation failed after directory was created. Manual cleanup may be required.`,
      [
        `Original error: ${err instanceof Error ? err.message : String(err)}`,
        `Check .maestro/missions/${missionId}/ for partial state`,
      ],
    );
  }
}

/** List all missions with optional status filter */
export async function listMissions(
  missionStore: MissionStorePort,
  filter?: { status?: string; limit?: number },
): Promise<readonly Mission[]> {
  const missions = await missionStore.list();
  let filtered = missions;

  if (filter?.status) {
    filtered = filtered.filter((m) => m.status === filter.status);
  }

  if (filter?.limit !== undefined) {
    if (!Number.isInteger(filter.limit) || filter.limit <= 0) {
      throw new MaestroError("Mission list limit must be a positive integer", [
        "Usage: maestro mission list --limit 10",
      ]);
    }
    filtered = filtered.slice(0, filter.limit);
  }

  return filtered;
}

/** Get a mission by ID */
export async function showMission(
  missionStore: MissionStorePort,
  missionId: string,
): Promise<Mission | undefined> {
  return await missionStore.get(missionId);
}

/** Approve a draft mission */
export async function approveMission(
  missionStore: MissionStorePort,
  missionId: string,
): Promise<Mission> {
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  // Validate transition
  assertMissionTransition(mission.status, "approved");

  const updated = await missionStore.update(missionId, { status: "approved" });
  if (!updated) {
    throw new MaestroError(`Failed to approve mission ${missionId}`);
  }

  return updated;
}

/** Reject a draft mission */
export async function rejectMission(
  missionStore: MissionStorePort,
  missionId: string,
): Promise<Mission> {
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  // Validate transition
  assertMissionTransition(mission.status, "rejected");

  const updated = await missionStore.update(missionId, { status: "rejected" });
  if (!updated) {
    throw new MaestroError(`Failed to reject mission ${missionId}`);
  }

  return updated;
}

/** Update mission status or metadata */
export async function updateMission(
  missionStore: MissionStorePort,
  missionId: string,
  input: UpdateMissionInput,
): Promise<Mission> {
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  // Validate status transition if provided
  if (input.status !== undefined && input.status !== mission.status) {
    assertMissionTransition(mission.status, input.status);
  }

  const updated = await missionStore.update(missionId, input);
  if (!updated) {
    throw new MaestroError(`Failed to update mission ${missionId}`);
  }

  return updated;
}

/** Get valid next states for a mission */
export function getValidMissionNextStates(mission: Mission): readonly string[] {
  return canTransitionMission(mission.status, "approved")
    ? canTransitionMission(mission.status, "rejected")
      ? ["approved", "rejected"]
      : canTransitionMission(mission.status, "executing")
        ? ["executing"]
        : canTransitionMission(mission.status, "validating")
          ? ["validating"]
          : canTransitionMission(mission.status, "completed")
            ? ["completed"]
            : canTransitionMission(mission.status, "failed")
              ? ["failed"]
              : []
    : [];
}
