export type EvidenceKind =
  | "command"
  | "manual-note"
  | "verifier"
  | "contract-amendment"
  | "contract-amendment-blocked"
  | "ai-review"
  | "plan-check"
  | "threat-model"
  | "review-ack"
  | "rollback-exercised"
  | "verdict-override"
  | "runtime-signal"
  | "deploy-readiness"
  | "cross-task-conflict"
  | "lint-violation"
  | "session-start"
  | "session-exit"
  | "recovery"
  | "doc-gardening"
  | "ralph-iteration"
  | "harness-delta";

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

export interface PlanCheckPayload {
  readonly planFileSha: string;
  readonly findings: readonly {
    readonly check: string;
    readonly severity: "info" | "warn" | "error";
    readonly message: string;
  }[];
  readonly errorCount: number;
  readonly warnCount: number;
}

export type ThreatModelResidualRisk = "low" | "medium" | "high";

export interface ThreatModelMitigation {
  readonly threat: string;
  readonly mitigation: string;
}

export interface ThreatModelPayload {
  readonly assets: readonly string[];
  readonly threatCategories: readonly string[];
  readonly mitigations: readonly ThreatModelMitigation[];
  readonly residualRisk: ThreatModelResidualRisk;
  readonly criterion_id?: string;
  readonly source_file?: string;
}

export interface ReviewAckPayload {
  readonly verdictId: string;
  readonly ackedBy: string;
  readonly criteria: readonly string[];
}

/**
 * Payload for rollback-exercised evidence.
 * Declaration only — the producer ships at L7.5.
 */
export interface RollbackExercisedPayload {
  readonly command: string;
  readonly exit: number;
}

/**
 * Payload for verdict-override evidence (L6.5).
 * Append-only audit record. The original Verdict is NOT rewritten.
 * Authorization: invoking user must be in owners.yaml `sensitive_waiver`
 * (loaded from base branch, not PR head — Rule 12).
 */
export interface VerdictOverridePayload {
  readonly verdictId: string;
  readonly overriddenBy: string;
  readonly reason: string;
}

export interface RuntimeSignalPayload {
  readonly signal_name: string;
  readonly provider: string;
  readonly query: string;
  readonly value: number;
  readonly threshold: number;
  readonly operator: string;
  readonly pass: boolean;
  readonly sampled_at: string;
  readonly note?: string;
}

export interface DeployReadinessPayload {
  readonly task_id: string;
  readonly checks: {
    readonly feature_flag: { readonly ok: boolean; readonly value?: string };
    readonly canary_plan: { readonly ok: boolean; readonly stages?: number };
    readonly rollback: { readonly ok: boolean; readonly witness_evidence_id?: string };
    readonly owner: { readonly ok: boolean; readonly approvers?: readonly string[] };
  };
  readonly gate: "pass" | "fail";
}

export interface CrossTaskConflictPayload {
  readonly thisPr: number;
  readonly conflictingPrs: readonly number[];
  readonly overlappingPaths: readonly string[];
}

/**
 * Payload for a single architecture-lint violation. Recorded by `task verify`,
 * `ci verify`, and `session start`/`session exit` whenever an architecture
 * rule fires at error severity. Kept queryable as its own kind so C-1's
 * `task introspect` can list "open lints" without parsing verifier-finding text.
 */
export interface LintViolationPayload {
  readonly ruleId: string;
  readonly file: string;
  readonly line?: number;
  readonly snippet?: string;
  readonly message: string;
  readonly remediation: string;
}

export interface SessionStartPayload {
  readonly taskId: string;
  readonly headSha: string;
}

export interface SessionExitPayload {
  readonly taskId: string;
  readonly lintViolations: number;
  readonly baselineClean: boolean;
  readonly dirtyTree: boolean;
}

export interface RecoveryPayload {
  readonly taskId: string;
  readonly fromCommit: string;
  readonly toCommit: string;
  readonly anchorVerdictId?: string;
  readonly droppedRunState: boolean;
  readonly reason: "verdict-anchored" | "explicit-ref" | "head-revert";
}

export interface DocGardeningPayload {
  readonly staleReferences: readonly {
    readonly file: string;
    readonly line: number;
    readonly reference: string;
    readonly kind: "missing-file" | "missing-symbol" | "moved-path" | "broken-link";
  }[];
  readonly scannedFiles: number;
  readonly prCreated?: number;
}

export interface RalphIterationPayload {
  readonly iteration: number;
  readonly findingsHash: string;
  readonly findingsCount: number;
  readonly stuck: boolean;
  readonly sources: readonly ("trust-verifier" | "ai-review" | "lint-arch" | "threat-model")[];
}

/**
 * Payload for harness-delta evidence. Records that a task modified the
 * development harness itself (policies, skills, hooks, .maestro/). Captured
 * at task close when `IntakeResult.harnessImpact` is true.
 */
export type HarnessDeltaCategory = "validation" | "workflow" | "policy";

export interface HarnessDeltaPayload {
  readonly paths: readonly string[];
  readonly category: HarnessDeltaCategory;
  readonly impactScope: string;
}

interface EvidencePayloadByKind {
  readonly command: CommandPayload;
  readonly "manual-note": ManualNotePayload;
  readonly verifier: VerifierPayload;
  readonly "contract-amendment": ContractAmendmentPayload;
  readonly "contract-amendment-blocked": ContractAmendmentBlockedPayload;
  readonly "ai-review": AIReviewPayload;
  readonly "plan-check": PlanCheckPayload;
  readonly "threat-model": ThreatModelPayload;
  readonly "review-ack": ReviewAckPayload;
  readonly "rollback-exercised": RollbackExercisedPayload;
  readonly "verdict-override": VerdictOverridePayload;
  readonly "runtime-signal": RuntimeSignalPayload;
  readonly "deploy-readiness": DeployReadinessPayload;
  readonly "cross-task-conflict": CrossTaskConflictPayload;
  readonly "lint-violation": LintViolationPayload;
  readonly "session-start": SessionStartPayload;
  readonly "session-exit": SessionExitPayload;
  readonly recovery: RecoveryPayload;
  readonly "doc-gardening": DocGardeningPayload;
  readonly "ralph-iteration": RalphIterationPayload;
  readonly "harness-delta": HarnessDeltaPayload;
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

/** Lean projection of {@link EvidenceRow} for list endpoints. */
export interface EvidenceSummary {
  readonly id: string;
  readonly task_id: string;
  readonly kind: EvidenceKind;
  readonly witness_level: WitnessLevel;
  readonly created_at: string;
  readonly session_id?: string;
}
