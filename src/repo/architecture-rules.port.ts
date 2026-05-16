export interface PassiveHarnessRules {
  readonly forbidden_patterns: readonly string[];
}

export interface ArchitectureRules {
  readonly version: 1;
  readonly forward_only: boolean;
  readonly layers: readonly string[];
  readonly cross_cutting: readonly string[];
  readonly lint_scope: readonly string[];
  readonly passive_harness: PassiveHarnessRules;
}

export interface ArchitectureRulesPort {
  load(): Promise<ArchitectureRules>;
}

export class ArchitectureRulesParseError extends Error {
  readonly field?: string;
  constructor(message: string, field?: string) {
    super(message);
    this.name = "ArchitectureRulesParseError";
    this.field = field;
  }
}

export class ArchitectureRulesNotFoundError extends Error {
  readonly path: string;
  constructor(path: string) {
    super(`Architecture rules file not found: ${path}`);
    this.name = "ArchitectureRulesNotFoundError";
    this.path = path;
  }
}
