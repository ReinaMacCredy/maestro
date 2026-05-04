import { join } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import { parseYaml } from "@/shared/lib/yaml.js";
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

export async function loadReleasePolicy(projectRoot: string): Promise<ReleasePolicy> {
  const filePath = join(projectRoot, RELEASE_POLICY_REL_PATH);
  const text = await readText(filePath);

  if (text === undefined) {
    return PERMISSIVE_DEFAULTS;
  }

  let raw: ReleasePolicyYaml;
  try {
    raw = parseYaml<ReleasePolicyYaml>(text) ?? {};
  } catch (err: unknown) {
    const yamlErr = err as { linePos?: Array<{ line: number }> };
    const line = yamlErr.linePos?.[0]?.line;
    const lineInfo = line !== undefined ? ` at line ${line}` : "";
    const msg = err instanceof Error ? err.message : String(err);
    throw new MaestroError(`release.yaml malformed${lineInfo}: ${msg}`, [
      "Fix the YAML syntax and re-run",
    ]);
  }

  if (typeof raw !== "object" || Array.isArray(raw)) {
    throw new MaestroError("release.yaml malformed: expected top-level object", [
      `Got ${Array.isArray(raw) ? "array" : typeof raw}`,
    ]);
  }

  const requireSignedCommits =
    raw.require_signed_commits === undefined
      ? false
      : typeof raw.require_signed_commits === "boolean"
        ? raw.require_signed_commits
        : (() => {
            throw new MaestroError(
              `release.yaml malformed: 'require_signed_commits' must be a boolean, got ${typeof raw.require_signed_commits}`,
              [],
            );
          })();

  const requireProofMapComplete =
    raw.require_proof_map_complete === undefined
      ? false
      : typeof raw.require_proof_map_complete === "boolean"
        ? raw.require_proof_map_complete
        : (() => {
            throw new MaestroError(
              `release.yaml malformed: 'require_proof_map_complete' must be a boolean, got ${typeof raw.require_proof_map_complete}`,
              [],
            );
          })();

  return {
    kind: "release",
    id: typeof raw.id === "string" ? raw.id : "release-policy-custom",
    version: typeof raw.version === "string" ? raw.version : "1",
    requireSignedCommits,
    requireProofMapComplete,
  };
}
