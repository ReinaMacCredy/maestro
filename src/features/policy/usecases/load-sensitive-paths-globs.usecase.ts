import { join } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import { parseYaml } from "@/shared/lib/yaml.js";

interface SensitivePathsYaml {
  readonly paths?: unknown;
}

/**
 * Load the sensitive-paths globs from `.maestro/policies/sensitive-paths.yaml`.
 * Returns `[]` when the file is absent or malformed (advisory: silent failure
 * is intentional for this advisory-level policy file).
 */
export async function loadSensitivePathsGlobs(projectRoot: string): Promise<readonly string[]> {
  const policyPath = join(projectRoot, ".maestro", "policies", "sensitive-paths.yaml");
  const raw = await readText(policyPath);
  if (raw === undefined) return [];
  try {
    const parsed = parseYaml<SensitivePathsYaml>(raw);
    return Array.isArray(parsed?.paths) ? (parsed.paths as string[]) : [];
  } catch {
    return [];
  }
}
