import type { RiskClass } from "@/features/task/index.js";

/**
 * Risk-flag taxonomy used by `maestro intake`. Modeled on the harness
 * "feature intake" risk checklist, mapped to maestro's existing diff signals
 * where possible.
 */
export type IntakeFlag =
  | "auth"
  | "authz"
  | "data-model"
  | "audit-security"
  | "external-systems"
  | "public-contracts"
  | "cross-platform"
  | "existing-behavior"
  | "weak-proof"
  | "multi-domain";

export type IntakeLane = "tiny" | "normal" | "high-risk";

/**
 * Work-type classification, ported from the harness-experimental framework.
 * Six mutually-exclusive categories shaped by intent rather than risk.
 *
 * Promoted from the `maestro-classify` skill in Phase 2 of the harness
 * integration. The skill remains for telemetry and explicit invocation.
 */
export type WorkType =
  | "new-spec"
  | "spec-slice"
  | "change-request"
  | "initiative"
  | "maintenance"
  | "harness-improvement";

export const WORK_TYPES: readonly WorkType[] = [
  "new-spec",
  "spec-slice",
  "change-request",
  "initiative",
  "maintenance",
  "harness-improvement",
] as const;

export interface IntakeInput {
  readonly intendedPaths: readonly string[];
  /** Flags the agent declares up front. Combined with auto-detected flags. */
  readonly declaredFlags?: readonly IntakeFlag[];
  /**
   * Manual override for the work-type classification. When provided, skips
   * heuristic classification and uses this value directly.
   */
  readonly declaredWorkType?: WorkType;
}

export interface IntakeResult {
  readonly lane: IntakeLane;
  readonly derivedRiskClass: RiskClass;
  readonly derivedRiskSignal: string | undefined;
  readonly autoDetectedFlags: readonly IntakeFlag[];
  readonly declaredFlags: readonly IntakeFlag[];
  readonly hardGatesTriggered: readonly IntakeFlag[];
  /**
   * True when the verdict pipeline will require a `threat-model` Evidence row
   * for a passing verdict. Mirrors the `requiresThreatModel` predicate in
   * compute-risk.ts so intake and verdict agree before code is written.
   */
  readonly threatModelRequired: boolean;
  readonly recommendedNextStep: string;
  /**
   * Work-type classification. Optional for backward compatibility with older
   * callers; populated whenever `classifyIntake` runs.
   */
  readonly workType?: WorkType;
  /**
   * True when any intended path falls under `.maestro/`, `policies/`,
   * `skills/`, or `hooks/`. Independent of `workType`.
   */
  readonly harnessImpact?: boolean;
  /**
   * Human-readable next-step hint derived from (workType, lane). Distinct from
   * `recommendedNextStep`, which is derived from lane alone.
   */
  readonly recommendedNextSteps?: string;
}
