import { join } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import { parseYaml } from "@/shared/lib/yaml.js";
import { MaestroError } from "@/shared/errors.js";
import type { Owners, OwnersYaml } from "../domain/owners-types.js";

const OWNERS_REL_PATH = ".maestro/policies/owners.yaml";

function isStringArray(val: unknown): val is readonly string[] {
  return Array.isArray(val) && val.every((v) => typeof v === "string");
}

function toRole(field: unknown, name: string): readonly string[] {
  if (field === undefined || field === null) return [];
  if (!isStringArray(field)) {
    throw new MaestroError("owners.yaml malformed", [
      `Expected '${name}' to be a list of strings, got ${typeof field}`,
    ]);
  }
  return field;
}

export async function loadOwners(baseDir: string): Promise<Owners> {
  const filePath = join(baseDir, OWNERS_REL_PATH);
  const text = await readText(filePath);

  if (text === undefined) {
    throw new MaestroError(
      `Owners file not found at ${OWNERS_REL_PATH}`,
      ["Run 'maestro init' to scaffold it"],
    );
  }

  let raw: OwnersYaml;
  try {
    raw = parseYaml<OwnersYaml>(text) ?? {};
  } catch {
    throw new MaestroError("owners.yaml malformed", ["YAML parse error"]);
  }

  if (typeof raw !== "object" || Array.isArray(raw)) {
    throw new MaestroError("owners.yaml malformed", [
      "Expected top-level object, got " + (Array.isArray(raw) ? "array" : typeof raw),
    ]);
  }

  return {
    policyApprovers: toRole(raw.policy_approver, "policy_approver"),
    ratchetApprovers: toRole(raw.ratchet_approver, "ratchet_approver"),
    sensitiveWaivers: toRole(raw.sensitive_waiver, "sensitive_waiver"),
  };
}
