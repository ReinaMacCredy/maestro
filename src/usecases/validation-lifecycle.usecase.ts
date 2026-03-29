/**
 * Validation lifecycle usecases
 * Implements assertion validation: show and update with milestone filtering,
 * evidence persistence, waive-with-reason handling, retry-to-pending from failed/blocked,
 * and helpful transition errors for invalid result changes.
 */
import type { AssertionStorePort } from "../ports/assertion-store.port.js";
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type {
  Assertion,
  UpdateAssertionInput,
} from "../domain/mission-types.js";
import { MaestroError } from "../domain/errors.js";
import { assertAssertionTransition } from "../domain/mission-state.js";

/** Result of showing assertions */
export interface ShowAssertionsResult {
  assertions: readonly Assertion[];
  total: number;
  filtered: number;
  milestoneId?: string;
  assertionCount: number;
}

/** Result of updating an assertion */
export interface UpdateAssertionResult {
  assertion: Assertion;
}

/**
 * Show assertions for a mission, optionally filtered by milestone
 */
export async function showAssertions(
  missionStore: MissionStorePort,
  assertionStore: AssertionStorePort,
  missionId: string,
  milestoneId?: string,
): Promise<ShowAssertionsResult> {
  // Verify mission exists
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  // Get assertions for the mission or specific milestone
  let assertions: readonly Assertion[];
  if (milestoneId) {
    assertions = await assertionStore.listByMilestone(missionId, milestoneId);
  } else {
    assertions = await assertionStore.list(missionId);
  }

  const totalAssertions = await assertionStore.list(missionId);

  return {
    assertions,
    total: totalAssertions.length,
    filtered: assertions.length,
    milestoneId,
    assertionCount: assertions.length,
  };
}

/**
 * Update an assertion's status with evidence and/or waived reason
 * Enforces legal state transitions and validates waive requirements
 */
export async function updateAssertion(
  missionStore: MissionStorePort,
  assertionStore: AssertionStorePort,
  missionId: string,
  assertionId: string,
  input: UpdateAssertionInput,
): Promise<UpdateAssertionResult> {
  // Verify mission exists
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  // Get existing assertion
  const existing = await assertionStore.get(missionId, assertionId);
  if (!existing) {
    throw new MaestroError(`Assertion ${assertionId} not found in mission ${missionId}`, [
      `List assertions: maestro validate show --mission ${missionId}`,
      `Check that assertion ID '${assertionId}' is correct`,
    ]);
  }

  // Validate status transition if provided and different
  if (input.status !== undefined && input.status !== existing.status) {
    assertAssertionTransition(existing.status, input.status);
  }

  // Build final update input, preserving existing evidence if not provided
  let finalEvidence = input.evidence;
  if (input.evidence === undefined && input.status !== undefined) {
    // When transitioning (especially retrying), preserve existing evidence
    finalEvidence = existing.evidence;
  }

  const updateInput: UpdateAssertionInput = {
    status: input.status,
    evidence: finalEvidence,
    waivedReason: input.waivedReason,
  };

  // Update the assertion
  const updated = await assertionStore.update(missionId, assertionId, updateInput);
  if (!updated) {
    throw new MaestroError(`Failed to update assertion ${assertionId}`);
  }

  return { assertion: updated };
}
