import { join } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import { parsePolicyYaml } from "@/shared/lib/yaml.js";
import { MaestroError } from "@/shared/errors.js";
import {
  WITNESS_LEVEL_ORDER,
  isWitnessLevel,
  type WitnessLevel,
} from "@/features/evidence/index.js";
import { RISK_CLASS_ORDER } from "@/features/risk/index.js";
import type { RiskClass } from "@/types/product-spec.js";
import type { AutopilotPolicy } from "../domain/policy-types.js";

const AUTOPILOT_POLICY_REL_PATH = ".maestro/policies/autopilot.yaml";

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

interface AutopilotPolicyYaml {
  readonly kind?: unknown;
  readonly id?: unknown;
  readonly version?: unknown;
  readonly auto_merge_allowed?: unknown;
  readonly required_witness_level?: unknown;
  [key: string]: unknown;
}

function assertKnownClassKey(key: string, fieldName: string): asserts key is RiskClass {
  if (!(RISK_CLASS_ORDER as readonly string[]).includes(key)) {
    throw new MaestroError(
      `autopilot.yaml malformed: unknown risk class key '${key}' in '${fieldName}'`,
      [`Valid keys are: ${RISK_CLASS_ORDER.join(", ")}`],
    );
  }
}

function parseBooleanMap(
  raw: unknown,
  fieldName: string,
): Record<RiskClass, boolean> {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    throw new MaestroError(`autopilot.yaml malformed: '${fieldName}' must be an object`, [
      `Expected an object with keys: ${RISK_CLASS_ORDER.join(", ")}`,
    ]);
  }

  const obj = raw as Record<string, unknown>;
  const result = {} as Record<RiskClass, boolean>;

  for (const key of RISK_CLASS_ORDER) {
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

  for (const key of Object.keys(obj)) {
    assertKnownClassKey(key, fieldName);
  }

  return result;
}

function parseWitnessMap(
  raw: unknown,
  fieldName: string,
): Record<RiskClass, WitnessLevel> {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    throw new MaestroError(`autopilot.yaml malformed: '${fieldName}' must be an object`, [
      `Expected an object with keys: ${RISK_CLASS_ORDER.join(", ")}`,
    ]);
  }

  const obj = raw as Record<string, unknown>;
  const result = {} as Record<RiskClass, WitnessLevel>;

  for (const key of RISK_CLASS_ORDER) {
    const val = obj[key];
    if (val === undefined) {
      result[key] = "witnessed-by-maestro";
    } else if (isWitnessLevel(val)) {
      result[key] = val;
    } else {
      throw new MaestroError(
        `autopilot.yaml malformed: '${fieldName}.${key}' is not a valid WitnessLevel: '${String(val)}'`,
        [`Valid values: ${WITNESS_LEVEL_ORDER.join(", ")}`],
      );
    }
  }

  for (const key of Object.keys(obj)) {
    assertKnownClassKey(key, fieldName);
  }

  return result;
}

export async function loadAutopilotPolicy(projectRoot: string): Promise<AutopilotPolicy> {
  const filePath = join(projectRoot, AUTOPILOT_POLICY_REL_PATH);
  const text = await readText(filePath);

  if (text === undefined) {
    return DISABLED_DEFAULTS;
  }

  const raw = parsePolicyYaml<AutopilotPolicyYaml>(text, "autopilot.yaml");

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
