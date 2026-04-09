// -- Corrections --

export interface CorrectionTrigger {
  readonly keywords: readonly string[];
  readonly fileGlobs: readonly string[];
}

export interface Correction {
  readonly id: string;
  readonly rule: string;
  readonly source: string;
  readonly trigger: CorrectionTrigger;
  readonly severity: "soft" | "hard";
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly promotedToRatchet?: string;
}

export interface CreateCorrectionInput {
  readonly rule: string;
  readonly source: string;
  readonly trigger: CorrectionTrigger;
  readonly severity: "soft" | "hard";
}

export interface CorrectionQuery {
  readonly keywords?: readonly string[];
  readonly filePaths?: readonly string[];
  readonly text?: string;
}

// -- Learnings --

export interface RawLearningEntry {
  readonly sessionDate: string;
  readonly content: string;
  readonly branch?: string;
}

export interface CompiledLearnings {
  readonly compiledAt: string;
  readonly summary: string;
  readonly rawCount: number;
}

// -- Stats (for TUI) --

export interface MemoryStats {
  readonly corrections: { readonly total: number; readonly hard: number; readonly soft: number };
  readonly learnings: { readonly rawCount: number; readonly compiledAt?: string; readonly staleDays?: number };
  readonly ratchet: { readonly assertions: number; readonly lastResult?: "pass" | "fail" };
  readonly graph: { readonly projects: number; readonly links: number };
}

// -- Config --

export interface MemoryConfig {
  readonly enabled: boolean;
  readonly corrections: {
    readonly enabled: boolean;
    readonly matching: "keyword" | "ast-grep" | "both";
    readonly auto_capture: "prompt" | "auto" | "off";
    readonly severity_default: "soft" | "hard";
  };
  readonly learnings: {
    readonly enabled: boolean;
    readonly compile_threshold: number;
    readonly max_age_days: number;
  };
  readonly ratchet: {
    readonly enabled: boolean;
    readonly enforcement: "warn" | "block";
  };
  readonly graph: {
    readonly enabled: boolean;
  };
}
