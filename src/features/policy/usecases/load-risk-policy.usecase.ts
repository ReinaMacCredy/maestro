import { join } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import { parsePolicyYaml } from "@/shared/lib/yaml.js";
import { MaestroError } from "@/shared/errors.js";
import type { RiskClass } from "@/types/product-spec.js";
import { RISK_CLASS_ORDER } from "@/features/risk/index.js";
import type { RiskPolicy, RiskPolicyRow } from "../domain/policy-types.js";
import { DEFAULT_RISK_POLICY } from "../domain/risk-policy-defaults.js";

const RISK_POLICY_REL_PATH = ".maestro/policies/risk.yaml";

const VALID_RISK_CLASSES = new Set<string>(RISK_CLASS_ORDER);

interface RiskPolicyRowYaml {
  readonly signal?: unknown;
  readonly derived_class?: unknown;
  readonly description?: unknown;
}

interface RiskPolicyYaml {
  readonly kind?: unknown;
  readonly id?: unknown;
  readonly description?: unknown;
  readonly version?: unknown;
  readonly rows?: unknown;
  [key: string]: unknown;
}

function isRiskClass(val: unknown): val is RiskClass {
  return typeof val === "string" && VALID_RISK_CLASSES.has(val);
}

function validateRow(row: unknown, index: number): RiskPolicyRow {
  if (typeof row !== "object" || row === null || Array.isArray(row)) {
    throw new MaestroError(`risk.yaml malformed at row ${index}: expected object`, [
      `Row ${index} is not an object`,
    ]);
  }

  const r = row as RiskPolicyRowYaml;

  if (typeof r.signal !== "string" || r.signal.trim() === "") {
    throw new MaestroError(`risk.yaml malformed at row ${index}: missing or invalid signal`, [
      `Row ${index} must have a non-empty string 'signal' field`,
    ]);
  }

  if (!isRiskClass(r.derived_class)) {
    throw new MaestroError(
      `risk.yaml malformed at row ${index}: unknown derived_class '${String(r.derived_class)}'`,
      [
        `unknown derived_class '${String(r.derived_class)}' — must be one of: ${RISK_CLASS_ORDER.join(", ")}`,
      ],
    );
  }

  return {
    signal: r.signal,
    derivedClass: r.derived_class,
    description: typeof r.description === "string" ? r.description : undefined,
  };
}

export async function loadRiskPolicy(projectRoot: string): Promise<RiskPolicy> {
  const filePath = join(projectRoot, RISK_POLICY_REL_PATH);
  const text = await readText(filePath);

  if (text === undefined) {
    return DEFAULT_RISK_POLICY;
  }

  const raw = parsePolicyYaml<RiskPolicyYaml>(text, "risk.yaml");

  if (!Array.isArray(raw.rows)) {
    throw new MaestroError("risk.yaml malformed: missing or invalid 'rows' array", [
      "'rows' must be a list of signal/derived_class entries",
    ]);
  }

  const rows = raw.rows.map((row, i) => validateRow(row, i));

  return {
    kind: "risk",
    id: typeof raw.id === "string" ? raw.id : "risk-policy-custom",
    description: typeof raw.description === "string" ? raw.description : undefined,
    version: typeof raw.version === "string" ? raw.version : "1",
    rows,
  };
}
