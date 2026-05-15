import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import { loadSensitivePathsGlobs } from "@/features/policy/index.js";
import type { TrustFinding } from "@/v2/types/trust.js";

/**
 * Loads globs from `.maestro/policies/sensitive-paths.yaml` and emits a
 * warn finding for each diff path matching a sensitive-path glob.
 * If the policy file is absent, returns empty findings (advisory only).
 */
export async function checkSensitivePaths(
  changedPaths: readonly string[],
  projectRoot: string,
): Promise<readonly TrustFinding[]> {
  const globs = await loadSensitivePathsGlobs(projectRoot);
  if (globs.length === 0) return [];

  const matched = changedPaths.filter((p) => matchesAnyGlob(globs, p));
  if (matched.length === 0) return [];

  return [
    {
      check: "sensitive-paths",
      severity: "warn",
      paths: matched,
      details: `${matched.length} path(s) match sensitive-path policy globs.`,
    },
  ];
}
