import type { Spec, AcceptanceCriterion, NonGoal } from "../domain/types.js";
import { generateCriterionId } from "../domain/spec-id.js";
import type { SpecStorePort } from "../ports/storage.js";

export interface CreateSpecInput {
  readonly mission_id: string;
  readonly acceptance_criteria: readonly { readonly text: string }[];
  readonly non_goals?: readonly { readonly text: string }[];
}

export async function createSpec(
  store: SpecStorePort,
  input: CreateSpecInput,
  now: () => string = () => new Date().toISOString(),
): Promise<Spec> {
  const timestamp = now();
  const criteria: AcceptanceCriterion[] = input.acceptance_criteria.map((c) => ({
    id: generateCriterionId(),
    text: c.text,
  }));
  const nonGoals: NonGoal[] = (input.non_goals ?? []).map((g) => ({ text: g.text }));
  const spec: Spec = {
    schema_version: 2,
    mission_id: input.mission_id,
    acceptance_criteria: criteria,
    non_goals: nonGoals,
    runtime_signals: [],
    created_at: timestamp,
    updated_at: timestamp,
  };
  await store.write(spec);
  return spec;
}
