import { parseYaml } from "@/shared/lib/yaml.js";
import { MaestroError } from "@/shared/errors.js";
import type { PolicyKind } from "../domain/policy-types.js";

export interface PolicyEdit {
  /** Human-readable description, e.g. "raised required witness for high from agent-claimed-locally to witnessed-by-maestro" */
  readonly description: string;
  /** Dotted-path into the YAML, e.g. "requiredWitnessLevel.high" */
  readonly path?: string;
  /** The value before the edit (for reversal) */
  readonly oldValue?: unknown;
  /** The value after the edit */
  readonly newValue?: unknown;
}

export interface PolicyEditClassification {
  readonly tightenings: readonly PolicyEdit[];
  readonly loosenings: readonly PolicyEdit[];
}

// WitnessLevel ladder: higher index = stronger/more restrictive
const WITNESS_LEVEL_ORDER = [
  "agent-claimed-and-not-reproducible",
  "agent-claimed-locally",
  "witnessed-by-ci",
  "witnessed-by-maestro",
] as const;
type WitnessLevel = (typeof WITNESS_LEVEL_ORDER)[number];

function witnessLevelScore(level: string): number {
  return WITNESS_LEVEL_ORDER.indexOf(level as WitnessLevel);
}

// Risk class ladder: higher index = more restrictive
const RISK_CLASS_ORDER = ["low", "medium", "high", "critical"] as const;
type RiskClass = (typeof RISK_CLASS_ORDER)[number];

function riskClassScore(cls: string): number {
  return RISK_CLASS_ORDER.indexOf(cls as RiskClass);
}

function parseOrEmpty<T>(yaml: string): T {
  if (!yaml.trim()) return {} as T;
  try {
    return (parseYaml<T>(yaml) ?? {}) as T;
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    throw new MaestroError(`Failed to parse policy YAML: ${msg}`, [
      "Fix the YAML syntax and re-run",
    ]);
  }
}

// --- risk.yaml ---

interface RiskRow {
  readonly signal: string;
  readonly derived_class: string;
}

interface RiskYaml {
  readonly rows?: readonly RiskRow[];
}

function classifyRiskEdit(oldYaml: string, newYaml: string): PolicyEditClassification {
  const oldParsed = parseOrEmpty<RiskYaml>(oldYaml);
  const newParsed = parseOrEmpty<RiskYaml>(newYaml);

  const oldRows: readonly RiskRow[] = Array.isArray(oldParsed?.rows) ? oldParsed.rows : [];
  const newRows: readonly RiskRow[] = Array.isArray(newParsed?.rows) ? newParsed.rows : [];

  const tightenings: PolicyEdit[] = [];
  const loosenings: PolicyEdit[] = [];

  // Build maps by signal
  const oldBySignal = new Map<string, RiskRow>();
  for (const row of oldRows) oldBySignal.set(row.signal, row);

  const newBySignal = new Map<string, RiskRow>();
  for (const row of newRows) newBySignal.set(row.signal, row);

  // Rows added → tightening
  for (const [signal, row] of newBySignal) {
    if (!oldBySignal.has(signal)) {
      tightenings.push({
        description: `added risk row: signal '${signal}' derived_class '${row.derived_class}'`,
        path: `rows[${signal}]`,
        oldValue: undefined,
        newValue: row,
      });
    }
  }

  // Rows removed → loosening
  for (const [signal, row] of oldBySignal) {
    if (!newBySignal.has(signal)) {
      loosenings.push({
        description: `removed risk row: signal '${signal}' (was derived_class '${row.derived_class}')`,
        path: `rows[${signal}]`,
        oldValue: row,
        newValue: undefined,
      });
    }
  }

  // Rows changed
  for (const [signal, newRow] of newBySignal) {
    const oldRow = oldBySignal.get(signal);
    if (!oldRow) continue; // already handled as added
    if (oldRow.derived_class === newRow.derived_class) continue;

    const oldScore = riskClassScore(oldRow.derived_class);
    const newScore = riskClassScore(newRow.derived_class);

    if (newScore > oldScore) {
      tightenings.push({
        description: `raised derived_class for signal '${signal}' from '${oldRow.derived_class}' to '${newRow.derived_class}'`,
        path: `rows[${signal}].derived_class`,
        oldValue: oldRow.derived_class,
        newValue: newRow.derived_class,
      });
    } else {
      loosenings.push({
        description: `lowered derived_class for signal '${signal}' from '${oldRow.derived_class}' to '${newRow.derived_class}'`,
        path: `rows[${signal}].derived_class`,
        oldValue: oldRow.derived_class,
        newValue: newRow.derived_class,
      });
    }
  }

  return { tightenings, loosenings };
}

// --- autopilot.yaml ---

interface AutopilotYaml {
  readonly auto_merge_allowed?: Record<string, boolean>;
  readonly required_witness_level?: Record<string, string>;
}

const RISK_CLASSES = ["low", "medium", "high", "critical"] as const;

function classifyAutopilotEdit(oldYaml: string, newYaml: string): PolicyEditClassification {
  const oldParsed = parseOrEmpty<AutopilotYaml>(oldYaml);
  const newParsed = parseOrEmpty<AutopilotYaml>(newYaml);

  const tightenings: PolicyEdit[] = [];
  const loosenings: PolicyEdit[] = [];

  const oldMerge = (oldParsed?.auto_merge_allowed ?? {}) as Record<string, boolean>;
  const newMerge = (newParsed?.auto_merge_allowed ?? {}) as Record<string, boolean>;
  const oldWitness = (oldParsed?.required_witness_level ?? {}) as Record<string, string>;
  const newWitness = (newParsed?.required_witness_level ?? {}) as Record<string, string>;

  for (const cls of RISK_CLASSES) {
    // autoMergeAllowed: false→true is loosening; true→false is tightening
    const oldMergeVal = oldMerge[cls] ?? false;
    const newMergeVal = newMerge[cls] ?? false;
    if (oldMergeVal !== newMergeVal) {
      if (!oldMergeVal && newMergeVal) {
        loosenings.push({
          description: `auto_merge_allowed.${cls}: false → true`,
          path: `autoMergeAllowed.${cls}`,
          oldValue: false,
          newValue: true,
        });
      } else {
        tightenings.push({
          description: `auto_merge_allowed.${cls}: true → false`,
          path: `autoMergeAllowed.${cls}`,
          oldValue: true,
          newValue: false,
        });
      }
    }

    // requiredWitnessLevel: higher score = stronger = tightening
    const oldWitnessVal = oldWitness[cls] ?? "witnessed-by-maestro";
    const newWitnessVal = newWitness[cls] ?? "witnessed-by-maestro";
    if (oldWitnessVal !== newWitnessVal) {
      const oldScore = witnessLevelScore(oldWitnessVal);
      const newScore = witnessLevelScore(newWitnessVal);
      if (newScore > oldScore) {
        tightenings.push({
          description: `required_witness_level.${cls} raised from '${oldWitnessVal}' to '${newWitnessVal}'`,
          path: `requiredWitnessLevel.${cls}`,
          oldValue: oldWitnessVal,
          newValue: newWitnessVal,
        });
      } else {
        loosenings.push({
          description: `required_witness_level.${cls} lowered from '${oldWitnessVal}' to '${newWitnessVal}'`,
          path: `requiredWitnessLevel.${cls}`,
          oldValue: oldWitnessVal,
          newValue: newWitnessVal,
        });
      }
    }
  }

  return { tightenings, loosenings };
}

// --- release.yaml ---

interface ReleaseYaml {
  readonly require_signed_commits?: boolean;
  readonly require_proof_map_complete?: boolean;
}

function classifyReleaseEdit(oldYaml: string, newYaml: string): PolicyEditClassification {
  const oldParsed = parseOrEmpty<ReleaseYaml>(oldYaml);
  const newParsed = parseOrEmpty<ReleaseYaml>(newYaml);

  const tightenings: PolicyEdit[] = [];
  const loosenings: PolicyEdit[] = [];

  const boolFields = [
    { field: "require_signed_commits", path: "requireSignedCommits" },
    { field: "require_proof_map_complete", path: "requireProofMapComplete" },
  ] as const;

  for (const { field, path } of boolFields) {
    const oldVal = (oldParsed as Record<string, boolean | undefined>)[field] ?? false;
    const newVal = (newParsed as Record<string, boolean | undefined>)[field] ?? false;
    if (oldVal !== newVal) {
      if (!oldVal && newVal) {
        tightenings.push({
          description: `${field}: false → true`,
          path,
          oldValue: false,
          newValue: true,
        });
      } else {
        loosenings.push({
          description: `${field}: true → false`,
          path,
          oldValue: true,
          newValue: false,
        });
      }
    }
  }

  return { tightenings, loosenings };
}

// --- sensitive-paths.yaml ---

interface SensitivePathsYaml {
  readonly globs?: readonly string[];
}

function classifySensitivePathsEdit(oldYaml: string, newYaml: string): PolicyEditClassification {
  const oldParsed = parseOrEmpty<SensitivePathsYaml>(oldYaml);
  const newParsed = parseOrEmpty<SensitivePathsYaml>(newYaml);

  const oldGlobs = new Set<string>(Array.isArray(oldParsed?.globs) ? oldParsed.globs : []);
  const newGlobs = new Set<string>(Array.isArray(newParsed?.globs) ? newParsed.globs : []);

  const tightenings: PolicyEdit[] = [];
  const loosenings: PolicyEdit[] = [];

  for (const glob of newGlobs) {
    if (!oldGlobs.has(glob)) {
      tightenings.push({
        description: `added sensitive-path glob: '${glob}'`,
        path: `globs[${glob}]`,
        oldValue: undefined,
        newValue: glob,
      });
    }
  }

  for (const glob of oldGlobs) {
    if (!newGlobs.has(glob)) {
      loosenings.push({
        description: `removed sensitive-path glob: '${glob}'`,
        path: `globs[${glob}]`,
        oldValue: glob,
        newValue: undefined,
      });
    }
  }

  return { tightenings, loosenings };
}

// --- Public entry point ---

export function classifyPolicyEdit(args: {
  readonly oldYaml: string;
  readonly newYaml: string;
  readonly kind: PolicyKind;
}): PolicyEditClassification {
  switch (args.kind) {
    case "risk":
      return classifyRiskEdit(args.oldYaml, args.newYaml);
    case "autopilot":
      return classifyAutopilotEdit(args.oldYaml, args.newYaml);
    case "release":
      return classifyReleaseEdit(args.oldYaml, args.newYaml);
    case "sensitive-paths":
      return classifySensitivePathsEdit(args.oldYaml, args.newYaml);
    case "owners":
      // owners changes are not safety policy — skip classification
      return { tightenings: [], loosenings: [] };
  }
}
