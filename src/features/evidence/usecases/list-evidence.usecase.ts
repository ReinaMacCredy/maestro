import type { EvidenceRow } from "../domain/types.js";
import type {
  EvidenceListFilter,
  EvidenceStorePort,
} from "../ports/storage.js";

export async function listEvidence(
  store: EvidenceStorePort,
  filter: EvidenceListFilter = {},
): Promise<readonly EvidenceRow[]> {
  return store.list(filter);
}
