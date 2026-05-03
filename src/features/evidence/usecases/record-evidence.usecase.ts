import { generateEvidenceId } from "../domain/evidence-id.js";
import type {
  EvidenceKind,
  EvidencePayload,
  EvidenceRow,
  WitnessLevel,
} from "../domain/types.js";
import type { EvidenceStorePort } from "../ports/storage.js";

export interface RecordEvidenceInput<K extends EvidenceKind = EvidenceKind> {
  readonly task_id: string;
  readonly session_id?: string;
  readonly kind: K;
  readonly payload: EvidencePayload<K>;
  readonly witness_level: WitnessLevel;
}

export async function recordEvidence<K extends EvidenceKind>(
  store: EvidenceStorePort,
  input: RecordEvidenceInput<K>,
): Promise<EvidenceRow<K>> {
  const row: EvidenceRow<K> = {
    schema_version: 1,
    id: generateEvidenceId(),
    task_id: input.task_id,
    session_id: input.session_id,
    kind: input.kind,
    witness_level: input.witness_level,
    created_at: new Date().toISOString(),
    payload: input.payload,
  };
  await store.append(row);
  return row;
}
