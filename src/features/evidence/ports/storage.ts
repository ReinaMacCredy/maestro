import type { EvidenceKind, EvidenceRow } from "../domain/types.js";

export interface EvidenceListFilter {
  readonly task_id?: string;
  readonly session_id?: string;
  readonly kind?: EvidenceKind;
}

export interface EvidenceStorePort {
  append(row: EvidenceRow): Promise<void>;
  read(id: string): Promise<EvidenceRow | undefined>;
  list(filter?: EvidenceListFilter): Promise<readonly EvidenceRow[]>;
}
