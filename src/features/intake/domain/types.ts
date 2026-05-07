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

export interface IntakeInput {
  readonly intendedPaths: readonly string[];
  /** Flags the agent declares up front. Combined with auto-detected flags. */
  readonly declaredFlags?: readonly IntakeFlag[];
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
}
