import type { Spec, AcceptanceCriterion, NonGoal } from "../domain/types.js";
import { generateCriterionId } from "../domain/spec-id.js";
import type { SpecStorePort } from "../ports/storage.js";
import { MaestroError } from "@/shared/errors.js";

export interface UpdateSpecInput {
  readonly acceptance_criteria?: readonly { readonly id?: string; readonly text: string }[];
  readonly non_goals?: readonly { readonly text: string }[];
}

/**
 * Update an existing Spec. Criteria that carry an existing `id` preserve their
 * id (stable across reads). Criteria without an `id` get a fresh generated id.
 * If no Spec exists for the mission, throws MaestroError.
 */
export async function updateSpec(
  store: SpecStorePort,
  missionId: string,
  input: UpdateSpecInput,
  now: () => string = () => new Date().toISOString(),
): Promise<Spec> {
  const existing = await store.read(missionId);
  if (!existing) {
    throw new MaestroError(`No Spec found for mission: ${missionId}`, [
      "Create a spec first with `maestro spec edit --mission <id>`",
    ]);
  }

  const criteria: AcceptanceCriterion[] = input.acceptance_criteria !== undefined
    ? input.acceptance_criteria.map((c) => ({
        id: c.id ?? generateCriterionId(),
        text: c.text,
      }))
    : [...existing.acceptance_criteria];

  const nonGoals: NonGoal[] = input.non_goals !== undefined
    ? input.non_goals.map((g) => ({ text: g.text }))
    : [...existing.non_goals];

  const updated: Spec = {
    ...existing,
    acceptance_criteria: criteria,
    non_goals: nonGoals,
    updated_at: now(),
  };
  await store.write(updated);
  return updated;
}
