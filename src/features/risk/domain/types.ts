import type { RiskClass } from "@/features/task/index.js";

export interface DerivedRiskInput {
  readonly changedPaths: readonly string[];
  readonly migrationPaths?: readonly string[];
  readonly sensitivePathsPolicy?: readonly string[];
}

export interface DerivedRiskResult {
  readonly class: RiskClass;
  readonly matchedRow: { readonly signal: string; readonly description?: string };
}
