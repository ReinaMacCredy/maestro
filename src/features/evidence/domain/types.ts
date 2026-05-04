export type EvidenceKind =
  | "command"
  | "manual-note"
  | "verifier"
  | "contract-amendment"
  | "contract-amendment-blocked"
  | "ai-review";

export type WitnessLevel =
  | "witnessed-by-maestro"
  | "witnessed-by-ci"
  | "agent-claimed-locally"
  | "agent-claimed-and-not-reproducible";

/**
 * Witness-level ladder, weakest to strongest. Higher index = more trustworthy.
 * Sole source of truth for witness-level ordering across the codebase.
 */
export const WITNESS_LEVEL_ORDER: readonly WitnessLevel[] = [
  "agent-claimed-and-not-reproducible",
  "agent-claimed-locally",
  "witnessed-by-ci",
  "witnessed-by-maestro",
];

const WITNESS_LEVEL_SET = new Set<string>(WITNESS_LEVEL_ORDER);

export function isWitnessLevel(value: unknown): value is WitnessLevel {
  return typeof value === "string" && WITNESS_LEVEL_SET.has(value);
}

export function compareWitnessLevel(a: WitnessLevel, b: WitnessLevel): -1 | 0 | 1 {
  const ai = WITNESS_LEVEL_ORDER.indexOf(a);
  const bi = WITNESS_LEVEL_ORDER.indexOf(b);
  if (ai < bi) return -1;
  if (ai > bi) return 1;
  return 0;
}

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

export type AIReviewerKind = "bug" | "security" | "architecture";

export interface AIReviewFinding {
  readonly severity: "info" | "warn" | "error";
  readonly message: string;
  readonly paths?: readonly string[];
  readonly suggestion?: string;
}

export interface AIReviewPayload {
  readonly reviewer: AIReviewerKind;
  readonly findings: readonly AIReviewFinding[];
  readonly confidence: number;
  readonly criterion_id?: string;
}

interface EvidencePayloadByKind {
  readonly command: CommandPayload;
  readonly "manual-note": ManualNotePayload;
  readonly verifier: VerifierPayload;
  readonly "contract-amendment": ContractAmendmentPayload;
  readonly "contract-amendment-blocked": ContractAmendmentBlockedPayload;
  readonly "ai-review": AIReviewPayload;
}

export type EvidencePayload<K extends EvidenceKind> = EvidencePayloadByKind[K];

export interface EvidenceRow<K extends EvidenceKind = EvidenceKind> {
  readonly schema_version: 3 | 2 | 1;
  readonly id: string;
  readonly task_id: string;
  readonly session_id?: string;
  readonly kind: K;
  readonly witness_level: WitnessLevel;
  readonly created_at: string;
  readonly payload: EvidencePayload<K>;
}
