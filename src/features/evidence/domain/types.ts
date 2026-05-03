export type EvidenceKind =
  | "command"
  | "manual-note"
  | "verifier"
  | "contract-amendment"
  | "contract-amendment-blocked";

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

export interface VerifierPayload {
  readonly check: string;
  readonly severity: "info" | "warn" | "error";
  readonly paths: readonly string[];
  readonly details?: string;
}

export interface ContractAmendmentPayload {
  readonly amendmentId: string;
  readonly addedPaths: readonly string[];
  readonly removedPaths: readonly string[];
  readonly reason: string;
}

export interface ContractAmendmentBlockedPayload {
  readonly reason: "budget_exhausted" | "forbidden_path" | "validation";
  readonly attemptedPaths: readonly string[];
  readonly details?: string;
}

interface EvidencePayloadByKind {
  readonly command: CommandPayload;
  readonly "manual-note": ManualNotePayload;
  readonly verifier: VerifierPayload;
  readonly "contract-amendment": ContractAmendmentPayload;
  readonly "contract-amendment-blocked": ContractAmendmentBlockedPayload;
}

export type EvidencePayload<K extends EvidenceKind> = EvidencePayloadByKind[K];

export interface EvidenceRow<K extends EvidenceKind = EvidenceKind> {
  readonly schema_version: 2 | 1;
  readonly id: string;
  readonly task_id: string;
  readonly session_id?: string;
  readonly kind: K;
  readonly witness_level: WitnessLevel;
  readonly created_at: string;
  readonly payload: EvidencePayload<K>;
}
