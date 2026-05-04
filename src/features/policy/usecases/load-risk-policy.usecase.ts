import { join } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import { parseYaml } from "@/shared/lib/yaml.js";
import { MaestroError } from "@/shared/errors.js";
import type { RiskClass } from "@/features/task/index.js";
import type { RiskPolicy, RiskPolicyRow } from "../domain/policy-types.js";
import { DEFAULT_RISK_POLICY } from "../domain/risk-policy-defaults.js";

const RISK_POLICY_REL_PATH = ".maestro/policies/risk.yaml";

const VALID_RISK_CLASSES = new Set<string>(["low", "medium", "high", "critical"]);

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
        `unknown derived_class '${String(r.derived_class)}' — must be one of: low, medium, high, critical`,
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

  let raw: RiskPolicyYaml;
  try {
    raw = parseYaml<RiskPolicyYaml>(text) ?? {};
  } catch (err: unknown) {
    const yamlErr = err as { linePos?: Array<{ line: number }> };
    const line = yamlErr.linePos?.[0]?.line;
    const lineInfo = line !== undefined ? ` at line ${line}` : "";
    const msg = err instanceof Error ? err.message : String(err);
    throw new MaestroError(`risk.yaml malformed${lineInfo}: ${msg}`, [
      "Fix the YAML syntax and re-run",
    ]);
  }

  if (typeof raw !== "object" || Array.isArray(raw)) {
    throw new MaestroError("risk.yaml malformed: expected top-level object", [
      `Got ${Array.isArray(raw) ? "array" : typeof raw}`,
    ]);
  }

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
