import { join } from "node:path";
import { parseYaml } from "@/shared/lib/yaml.js";
import { matchGlob } from "@/shared/lib/glob-match.js";
import { readText } from "@/shared/lib/fs.js";
import type { TrustFinding } from "../../domain/types.js";

interface SensitivePathsYaml {
  readonly paths?: readonly string[];
}

/**
 * Loads globs from `.maestro/policies/sensitive-paths.yaml` and emits a
 * warn finding for each diff path matching a sensitive-path glob.
 * If the policy file is absent, returns empty findings (advisory only).
 */
export async function checkSensitivePaths(
  changedPaths: readonly string[],
  projectRoot: string,
): Promise<readonly TrustFinding[]> {
  const policyPath = join(projectRoot, ".maestro", "policies", "sensitive-paths.yaml");
  const raw = await readText(policyPath);
  if (raw === undefined) {
    return [];
  }

  let globs: readonly string[];
  try {
    const parsed = parseYaml<SensitivePathsYaml>(raw);
    globs = Array.isArray(parsed?.paths) ? parsed.paths : [];
  } catch {
    return [];
  }

  if (globs.length === 0) {
    return [];
  }

  const matched = changedPaths.filter((p) =>
    globs.some((g) => matchGlob(g, p)),
  );

  if (matched.length === 0) {
    return [];
  }

  return [
    {
      check: "sensitive-paths",
      severity: "warn",
      paths: matched,
      details: `${matched.length} path(s) match sensitive-path policy globs.`,
    },
  ];
}
