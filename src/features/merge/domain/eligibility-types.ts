export interface EligibilityResult {
  readonly eligible: boolean;
  readonly reasons: readonly EligibilityReason[];
}

export interface EligibilityReason {
  readonly code:
    | "verdict-not-pass"
    | "auto-merge-class-disabled"
    | "evidence-witness-too-weak"
    | "forbidden-paths-touched"
    | "sensitive-paths-untouched-without-waiver"
    | "rollback-not-witnessed"
    | "review-ack-missing"
    | "spec-score-below-threshold";
  readonly message: string;
  readonly evidenceIds?: readonly string[];
}
