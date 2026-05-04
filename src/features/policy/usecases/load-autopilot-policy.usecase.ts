import { join } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import { parseYaml } from "@/shared/lib/yaml.js";
import { MaestroError } from "@/shared/errors.js";
import type { WitnessLevel } from "@/features/evidence/index.js";
import type { AutopilotPolicy } from "../domain/policy-types.js";

const AUTOPILOT_POLICY_REL_PATH = ".maestro/policies/autopilot.yaml";

const VALID_RISK_CLASSES = ["low", "medium", "high", "critical"] as const;
type RiskClassKey = (typeof VALID_RISK_CLASSES)[number];

const VALID_WITNESS_LEVELS = new Set<string>([
  "witnessed-by-maestro",
  "witnessed-by-ci",
  "agent-claimed-locally",
  "agent-claimed-and-not-reproducible",
]);

const DISABLED_DEFAULTS: AutopilotPolicy = {
  kind: "autopilot",
  id: "autopilot-policy-default",
  version: "1",
  autoMergeAllowed: {
    low: false,
    medium: false,
    high: false,
    critical: false,
  },
  requiredWitnessLevel: {
    low: "witnessed-by-maestro",
    medium: "witnessed-by-maestro",
    high: "witnessed-by-maestro",
    critical: "witnessed-by-maestro",
  },
};

function isWitnessLevel(val: unknown): val is WitnessLevel {
  return typeof val === "string" && VALID_WITNESS_LEVELS.has(val);
}

interface AutopilotPolicyYaml {
  readonly kind?: unknown;
  readonly id?: unknown;
  readonly version?: unknown;
  readonly auto_merge_allowed?: unknown;
  readonly required_witness_level?: unknown;
}

function parseBooleanMap(
  raw: unknown,
  fieldName: string,
): Record<RiskClassKey, boolean> {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    throw new MaestroError(`autopilot.yaml malformed: '${fieldName}' must be an object`, [
      `Expected an object with keys: ${VALID_RISK_CLASSES.join(", ")}`,
    ]);
  }

  const obj = raw as Record<string, unknown>;
  const result = {} as Record<RiskClassKey, boolean>;

  for (const key of VALID_RISK_CLASSES) {
    const val = obj[key];
    if (val === undefined) {
      result[key] = false;
    } else if (typeof val === "boolean") {
      result[key] = val;
    } else {
      throw new MaestroError(
        `autopilot.yaml malformed: '${fieldName}.${key}' must be a boolean, got ${typeof val}`,
        [],
      );
    }
  }

  // Check for unknown class keys
  for (const key of Object.keys(obj)) {
    if (!VALID_RISK_CLASSES.includes(key as RiskClassKey)) {
      throw new MaestroError(
        `autopilot.yaml malformed: unknown risk class key '${key}' in '${fieldName}'`,
        [`Valid keys are: ${VALID_RISK_CLASSES.join(", ")}`],
      );
    }
  }

  return result;
}

function parseWitnessMap(
  raw: unknown,
  fieldName: string,
): Record<RiskClassKey, WitnessLevel> {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    throw new MaestroError(`autopilot.yaml malformed: '${fieldName}' must be an object`, [
      `Expected an object with keys: ${VALID_RISK_CLASSES.join(", ")}`,
    ]);
  }

  const obj = raw as Record<string, unknown>;
  const result = {} as Record<RiskClassKey, WitnessLevel>;

  for (const key of VALID_RISK_CLASSES) {
    const val = obj[key];
    if (val === undefined) {
      result[key] = "witnessed-by-maestro";
    } else if (isWitnessLevel(val)) {
      result[key] = val;
    } else {
      throw new MaestroError(
        `autopilot.yaml malformed: '${fieldName}.${key}' is not a valid WitnessLevel: '${String(val)}'`,
        [`Valid values: ${Array.from(VALID_WITNESS_LEVELS).join(", ")}`],
      );
    }
  }

  // Check for unknown class keys
  for (const key of Object.keys(obj)) {
    if (!VALID_RISK_CLASSES.includes(key as RiskClassKey)) {
      throw new MaestroError(
        `autopilot.yaml malformed: unknown risk class key '${key}' in '${fieldName}'`,
        [`Valid keys are: ${VALID_RISK_CLASSES.join(", ")}`],
      );
    }
  }

  return result;
}

export async function loadAutopilotPolicy(projectRoot: string): Promise<AutopilotPolicy> {
  const filePath = join(projectRoot, AUTOPILOT_POLICY_REL_PATH);
  const text = await readText(filePath);

  if (text === undefined) {
    return DISABLED_DEFAULTS;
  }

  let raw: AutopilotPolicyYaml;
  try {
    raw = parseYaml<AutopilotPolicyYaml>(text) ?? {};
  } catch (err: unknown) {
    const yamlErr = err as { linePos?: Array<{ line: number }> };
    const line = yamlErr.linePos?.[0]?.line;
    const lineInfo = line !== undefined ? ` at line ${line}` : "";
    const msg = err instanceof Error ? err.message : String(err);
    throw new MaestroError(`autopilot.yaml malformed${lineInfo}: ${msg}`, [
      "Fix the YAML syntax and re-run",
    ]);
  }

  if (typeof raw !== "object" || Array.isArray(raw)) {
    throw new MaestroError("autopilot.yaml malformed: expected top-level object", [
      `Got ${Array.isArray(raw) ? "array" : typeof raw}`,
    ]);
  }

  const autoMergeAllowed =
    raw.auto_merge_allowed !== undefined
      ? parseBooleanMap(raw.auto_merge_allowed, "auto_merge_allowed")
      : { ...DISABLED_DEFAULTS.autoMergeAllowed };

  const requiredWitnessLevel =
    raw.required_witness_level !== undefined
      ? parseWitnessMap(raw.required_witness_level, "required_witness_level")
      : { ...DISABLED_DEFAULTS.requiredWitnessLevel };

  return {
    kind: "autopilot",
    id: typeof raw.id === "string" ? raw.id : "autopilot-policy-custom",
    version: typeof raw.version === "string" ? raw.version : "1",
    autoMergeAllowed,
    requiredWitnessLevel,
  };
}
