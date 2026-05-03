export type Severity = "info" | "warn" | "error";

export interface TrustFinding {
  readonly check: string;
  readonly severity: Severity;
  readonly paths: readonly string[];
  readonly details?: string;
}

export interface TrustVerifierResult {
  readonly findings: readonly TrustFinding[];
}
