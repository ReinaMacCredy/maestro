export type EvidenceKind = "command" | "manual-note";

export type WitnessLevel =
  | "witnessed-by-maestro"
  | "witnessed-by-ci"
  | "agent-claimed-locally"
  | "agent-claimed-and-not-reproducible";

export interface CommandPayload {
  readonly command: string;
  readonly exit: number;
  readonly log_path?: string;
  readonly duration_ms?: number;
  readonly criterion_id?: string;
}

export interface ManualNotePayload {
  readonly note: string;
  readonly criterion_id?: string;
}

interface EvidencePayloadByKind {
  readonly command: CommandPayload;
  readonly "manual-note": ManualNotePayload;
}

export type EvidencePayload<K extends EvidenceKind> = EvidencePayloadByKind[K];

export interface EvidenceRow<K extends EvidenceKind = EvidenceKind> {
  readonly schema_version: 1;
  readonly id: string;
  readonly task_id: string;
  readonly session_id?: string;
  readonly kind: K;
  readonly witness_level: WitnessLevel;
  readonly created_at: string;
  readonly payload: EvidencePayload<K>;
}
