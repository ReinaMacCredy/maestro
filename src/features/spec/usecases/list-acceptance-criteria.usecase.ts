import type { AcceptanceCriterion } from "../domain/types.js";
import type { SpecStorePort } from "../ports/storage.js";

export async function listAcceptanceCriteria(
  store: SpecStorePort,
  missionId: string,
): Promise<readonly AcceptanceCriterion[]> {
  const spec = await store.read(missionId);
  return spec?.acceptance_criteria ?? [];
}
