// v2 product-spec frontmatter shapes (ADR-0010 document-driven workflow).
// The spec markdown file is the source of truth for both light and heavy paths.

export const WORK_TYPES = [
  "new-spec",
  "spec-slice",
  "change-request",
  "initiative",
  "maintenance",
  "harness-improvement",
] as const;

export type WorkType = (typeof WORK_TYPES)[number];

export function isWorkType(value: unknown): value is WorkType {
  return typeof value === "string" && (WORK_TYPES as readonly string[]).includes(value);
}

export const RISK_CLASSES = ["low", "medium", "high", "critical"] as const;
export type RiskClass = (typeof RISK_CLASSES)[number];

export function isRiskClass(value: unknown): value is RiskClass {
  return typeof value === "string" && (RISK_CLASSES as readonly string[]).includes(value);
}

export const SPEC_MODES = ["light", "heavy"] as const;
export type SpecMode = (typeof SPEC_MODES)[number];

export function isSpecMode(value: unknown): value is SpecMode {
  return typeof value === "string" && (SPEC_MODES as readonly string[]).includes(value);
}

export interface ProductSpecFrontmatter {
  readonly slug: string;
  readonly acceptance_criteria: readonly string[];
  readonly non_goals: readonly string[];
  readonly risk_class: RiskClass;
  readonly mode: SpecMode;
  readonly work_type: WorkType;
}

export interface ProductSpec {
  readonly frontmatter: ProductSpecFrontmatter;
  readonly body: string;
  readonly path: string;
}
