import { cp, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { dirExists } from "@/shared/lib/fs.js";

export interface BackupResult {
  readonly source: string;
  readonly destination: string;
  readonly created: boolean;
}

// Copy .maestro/ to .maestro.backup-<timestamp>/ so the migration can be
// rolled back manually. Returns created=false if the source does not exist.
export async function backupMaestroDir(
  repoRoot: string,
  timestamp: string,
): Promise<BackupResult> {
  const source = join(repoRoot, ".maestro");
  const stamp = sanitizeTimestamp(timestamp);
  const destination = join(repoRoot, `.maestro.backup-${stamp}`);
  if (!(await dirExists(source))) {
    return { source, destination, created: false };
  }
  await mkdir(destination, { recursive: true });
  await cp(source, destination, { recursive: true, errorOnExist: false, force: true });
  return { source, destination, created: true };
}

function sanitizeTimestamp(ts: string): string {
  return ts.replace(/[:.]/g, "-");
}
