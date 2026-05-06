import { join } from "node:path";
import { execFileSync } from "node:child_process";
import { readText } from "@/shared/lib/fs.js";
import { parseYaml } from "@/shared/lib/yaml.js";
import { MaestroError } from "@/shared/errors.js";
import type { Owners, OwnersYaml } from "../domain/owners-types.js";

export const OWNERS_REL_PATH = ".maestro/policies/owners.yaml";

const EMPTY_TREE_SHA = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

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

export function parseOwners(text: string): Owners {
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
    deployApprovers: toRole(raw.deploy_approver, "deploy_approver"),
  };
}

export function loadOwnersFromBase(base: string, projectRoot: string): Owners {
  let text: string;
  try {
    text = execFileSync(
      "git",
      ["show", `${base}:${OWNERS_REL_PATH}`],
      { cwd: projectRoot, encoding: "utf8", stdio: ["pipe", "pipe", "pipe"] },
    ).trim();
  } catch {
    if (base === EMPTY_TREE_SHA) {
      throw new MaestroError(
        "owners.yaml cannot be loaded from the empty-tree base",
        [
          "No upstream tracking branch and no merge-base with main/master/trunk was found,",
          "so the default base resolved to the empty tree (no files committed yet).",
          "Pass --base <commit-or-ref> explicitly to load owners.yaml from a real commit.",
        ],
      );
    }
    throw new MaestroError(
      `owners.yaml not found at ${base}:${OWNERS_REL_PATH}`,
      ["Run 'maestro init' to scaffold it, or check the base ref is correct"],
    );
  }
  return parseOwners(text);
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

  return parseOwners(text);
}
