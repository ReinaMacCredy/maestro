import { join } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import { parsePolicyYaml } from "@/shared/lib/yaml.js";
import { MaestroError } from "@/shared/errors.js";
import type { ReleasePolicy } from "../domain/policy-types.js";

const RELEASE_POLICY_REL_PATH = ".maestro/policies/release.yaml";

const PERMISSIVE_DEFAULTS: ReleasePolicy = {
  kind: "release",
  id: "release-policy-default",
  version: "1",
  requireSignedCommits: false,
  requireProofMapComplete: false,
};

interface ReleasePolicyYaml {
  readonly kind?: unknown;
  readonly id?: unknown;
  readonly version?: unknown;
  readonly require_signed_commits?: unknown;
  readonly require_proof_map_complete?: unknown;
}

function parseBooleanField(raw: unknown, fieldName: string): boolean {
  if (raw === undefined) return false;
  if (typeof raw === "boolean") return raw;
  throw new MaestroError(
    `release.yaml malformed: '${fieldName}' must be a boolean, got ${typeof raw}`,
    [],
  );
}

export async function loadReleasePolicy(projectRoot: string): Promise<ReleasePolicy> {
  const filePath = join(projectRoot, RELEASE_POLICY_REL_PATH);
  const text = await readText(filePath);

  if (text === undefined) {
    return PERMISSIVE_DEFAULTS;
  }

  const raw = parsePolicyYaml<ReleasePolicyYaml>(text, "release.yaml");

  return {
    kind: "release",
    id: typeof raw.id === "string" ? raw.id : "release-policy-custom",
    version: typeof raw.version === "string" ? raw.version : "1",
    requireSignedCommits: parseBooleanField(raw.require_signed_commits, "require_signed_commits"),
    requireProofMapComplete: parseBooleanField(
      raw.require_proof_map_complete,
      "require_proof_map_complete",
    ),
  };
}
