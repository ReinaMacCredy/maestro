// Principles are named golden rules that the harness exposes to agents.
// Each principle ships as a markdown file at docs/principles/<slug>.md and
// carries a scan command (used by `gc slop-cleanup`) plus a fix recipe.

export interface Principle {
  readonly slug: string;
  readonly rule: string;
  readonly rationale: string;
  readonly scan_command: string;
  readonly fix_recipe: string;
}

export const PRINCIPLE_SLUG_PATTERN = /^[a-z0-9][a-z0-9-]*[a-z0-9]$/;

export function isValidPrincipleSlug(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length >= 2 &&
    value.length <= 64 &&
    PRINCIPLE_SLUG_PATTERN.test(value)
  );
}

export class PrincipleParseError extends Error {
  readonly section?: string;
  constructor(message: string, section?: string) {
    super(message);
    this.name = "PrincipleParseError";
    this.section = section;
  }
}
