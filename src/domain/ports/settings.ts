/**
 * MaestroSettings: layered settings system (DEFAULTS -> global -> project).
 *
 * Supersedes HiveConfig for new consumers. Old config.json is auto-migrated
 * at read time (no file rewrite) by the FsSettingsAdapter.
 */

// ============================================================================
// Settings Types
// ============================================================================

export interface ToolboxSettings {
  allow: string[];
  deny: string[];
  config: Record<string, Record<string, unknown>>;
}

export interface AgentToolsSettings {
  allow: string[];
  deny: string[];
  config: Record<string, Record<string, unknown>>;
}

export interface DcpSettings {
  enabled: boolean;
  memoryBudgetTokens: number;
  completedTaskBudgetTokens: number;
  relevanceThreshold: number;
  observationMasking: boolean;
  handoffDecisionBudgetTokens: number;
}

export interface VerificationSettings {
  enabled: boolean;
  autoReject: boolean;
  maxRevisions: number;
  scoreThreshold: number;
  buildTimeoutMs: number;
  buildCommand?: string;
  autoAcceptTypes: string[];
}

export interface DoctrineSettings {
  enabled: boolean;
  doctrineBudgetTokens: number;
  maxSuggestionsPerFeature: number;
  crossFeatureScanLimit: number;
  minSampleSize: number;
  staleThresholdDays: number;
}

export interface TasksSettings {
  claimExpiresMinutes: number;
  backend: 'fs' | 'br' | 'auto';
}

export interface AgentModelSettings {
  model?: string;
  temperature?: number;
  skills?: string[];
  autoLoadSkills?: string[];
  variant?: string;
}

export interface HostSettings {
  /** Auto-detect host environment from env vars. */
  autoDetect: boolean;
  /** Override host type. 'auto' uses detection. */
  type: 'claude-code' | 'codex' | 'standalone' | 'auto';
  /** Mirror task state to host system (requires adapter). */
  mirror: boolean;
  /** Reconcile host tasks from filesystem on session start. */
  reconcileOnStart: boolean;
}

export interface MaestroSettings {
  toolbox: ToolboxSettings;
  agentTools: AgentToolsSettings;
  dcp: DcpSettings;
  verification: VerificationSettings;
  doctrine: DoctrineSettings;
  tasks: TasksSettings;
  agents: Record<string, AgentModelSettings>;
  host: HostSettings;
}

// ============================================================================
// Defaults
// ============================================================================

export const DEFAULT_SETTINGS: MaestroSettings = {
  toolbox: { allow: [], deny: [], config: {} },
  agentTools: { allow: [], deny: [], config: {} },
  dcp: {
    enabled: true,
    memoryBudgetTokens: 1024,
    completedTaskBudgetTokens: 512,
    relevanceThreshold: 0.1,
    observationMasking: true,
    handoffDecisionBudgetTokens: 512,
  },
  verification: {
    enabled: true,
    autoReject: true,
    maxRevisions: 2,
    scoreThreshold: 0.7,
    buildTimeoutMs: 30000,
    autoAcceptTypes: [],
  },
  doctrine: {
    enabled: true,
    doctrineBudgetTokens: 256,
    maxSuggestionsPerFeature: 5,
    crossFeatureScanLimit: 20,
    minSampleSize: 5,
    staleThresholdDays: 30,
  },
  tasks: {
    claimExpiresMinutes: 120,
    backend: 'auto',
  },
  agents: {},
  host: {
    autoDetect: true,
    type: 'auto',
    mirror: false,
    reconcileOnStart: false,
  },
};

// ============================================================================
// Port
// ============================================================================

export interface SettingsPort {
  get(): MaestroSettings;
  getToolConfig(name: string): Record<string, unknown>;
}

// ============================================================================
// Merge
// ============================================================================

/**
 * Deep-merge settings layers. Arrays replace (not concat). Undefined values
 * in overlay are skipped; null values delete the key.
 */
export function mergeSettings(
  base: MaestroSettings,
  ...overlays: Array<Partial<MaestroSettings>>
): MaestroSettings {
  let result = { ...base };

  for (const overlay of overlays) {
    result = mergeSettingsOnce(result, overlay);
  }

  return result;
}

function mergeSettingsOnce(
  base: MaestroSettings,
  overlay: Partial<MaestroSettings>,
): MaestroSettings {
  const result = { ...base };

  for (const key of Object.keys(overlay) as Array<keyof MaestroSettings>) {
    const val = overlay[key];
    if (val === undefined) continue;

    const baseVal = result[key];
    if (
      val !== null &&
      typeof val === 'object' &&
      !Array.isArray(val) &&
      baseVal !== null &&
      typeof baseVal === 'object' &&
      !Array.isArray(baseVal)
    ) {
      // Shallow merge for each section -- sections are flat (no nested objects)
      // except toolbox.config / agentTools.config which are Record<string, Record<...>>
      (result as Record<string, unknown>)[key] = mergeSection(
        baseVal as Record<string, unknown>,
        val as Record<string, unknown>,
      );
    } else {
      (result as Record<string, unknown>)[key] = val;
    }
  }

  return result;
}

function mergeSection(
  base: Record<string, unknown>,
  overlay: Record<string, unknown>,
): Record<string, unknown> {
  const result = { ...base };

  for (const key of Object.keys(overlay)) {
    const val = overlay[key];
    if (val === undefined) continue;

    const baseVal = result[key];
    if (
      val !== null &&
      typeof val === 'object' &&
      !Array.isArray(val) &&
      baseVal !== null &&
      typeof baseVal === 'object' &&
      !Array.isArray(baseVal)
    ) {
      // Recurse for nested objects (e.g. toolbox.config.br)
      result[key] = mergeSection(
        baseVal as Record<string, unknown>,
        val as Record<string, unknown>,
      );
    } else {
      result[key] = val;
    }
  }

  return result;
}

// ============================================================================
// Allow / Deny
// ============================================================================

/**
 * Determine if a tool is allowed given allow/deny lists.
 * - deny always wins
 * - non-empty allow = allowlist mode (only listed tools are allowed)
 * - empty allow + empty deny = everything allowed
 */
export function isToolAllowed(
  name: string,
  settings: { allow: string[]; deny: string[] },
): boolean {
  if (settings.deny.includes(name)) return false;
  if (settings.allow.length > 0) return settings.allow.includes(name);
  return true;
}
