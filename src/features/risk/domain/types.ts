import type { RiskClass } from "@/features/task/index.js";

export interface DerivedRiskInput {
  readonly changedPaths: readonly string[];
  readonly addedLines: readonly { readonly path: string; readonly lines: readonly string[] }[];
  readonly manifestChanges?: boolean;
  readonly migrationPaths?: readonly string[];
  readonly ciWorkflowChanges?: boolean;
  readonly sensitivePathsPolicy?: readonly string[];
}

export interface DerivedRiskResult {
  readonly class: RiskClass;
  readonly matchedRow: { readonly signal: string; readonly description?: string };
}
