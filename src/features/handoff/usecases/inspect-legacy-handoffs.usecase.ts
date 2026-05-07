import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";

// Kept for back-compat with infra callers that still pass `{ homeDir }`.
// The scan no longer reaches the home dir; the option is ignored.
export interface CountLegacyHandoffFilesOptions {
  readonly homeDir?: string;
}

/**
 * Counts legacy handoff/launch artifacts under the project's `.maestro/`
 * directory. Scoped to project state so doctor's per-project output
 * doesn't bleed in home-dir launches that belong to other repos.
 */
export async function countLegacyHandoffFiles(
  projectDir: string,
  _options: CountLegacyHandoffFilesOptions = {},
): Promise<number> {
  return (
    await Promise.all([
      countEntries(join(projectDir, MAESTRO_DIR, "handoffs")),
      countEntries(join(projectDir, MAESTRO_DIR, "launches")),
    ])
  ).reduce((sum, count) => sum + count, 0);
}

async function countEntries(dir: string): Promise<number> {
  try {
    const entries = await readdir(dir, { withFileTypes: true });
    return entries.filter((entry) => entry.isFile() || entry.isDirectory()).length;
  } catch {
    return 0;
  }
}
