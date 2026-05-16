import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";

// Kept for back-compat with infra callers that still pass `{ homeDir }`.
// The scan no longer reaches the home dir; the option is ignored.
export interface CountLegacyHandoffFilesOptions {
  readonly homeDir?: string;
}

/**
 * Counts legacy launch artifacts under the project's `.maestro/launches/`
 * directory. `.maestro/handoffs/` is the canonical emit path for the
 * current handoff system and is not counted here.
 */
export async function countLegacyHandoffFiles(
  projectDir: string,
  _options: CountLegacyHandoffFilesOptions = {},
): Promise<number> {
  return countEntries(join(projectDir, MAESTRO_DIR, "launches"));
}

async function countEntries(dir: string): Promise<number> {
  try {
    const entries = await readdir(dir, { withFileTypes: true });
    return entries.filter((entry) => entry.isFile() || entry.isDirectory()).length;
  } catch {
    return 0;
  }
}
