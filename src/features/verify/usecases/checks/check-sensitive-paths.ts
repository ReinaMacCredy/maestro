import { join } from "node:path";
import { parseYaml } from "@/shared/lib/yaml.js";
import { normalizeSlashes } from "@/shared/lib/path-normalize.js";
import type { TrustFinding } from "../../domain/types.js";

interface SensitivePathsYaml {
  readonly paths?: readonly string[];
}

// Per-pattern Glob cache mirrors the pattern from verdict.ts.
const globCache = new Map<string, Bun.Glob>();

function matchGlob(pattern: string, path: string): boolean {
  const normalized = normalizeSlashes(pattern);
  let glob = globCache.get(normalized);
  if (!glob) {
    glob = new Bun.Glob(normalized);
    globCache.set(normalized, glob);
  }
  return glob.match(path);
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
  const file = Bun.file(policyPath);
  if (!(await file.exists())) {
    return [];
  }

  let globs: readonly string[];
  try {
    const raw = await file.text();
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
