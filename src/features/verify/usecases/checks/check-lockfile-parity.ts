import { join } from "node:path";
import type { TrustFinding } from "../../domain/types.js";

const LOCKFILE_PAIRS: Array<readonly [manifest: string, lockfile: string]> = [
  ["package.json", "bun.lock"],
  ["package.json", "package-lock.json"],
  ["package.json", "pnpm-lock.yaml"],
];

/**
 * Enforces lockfile parity: if package.json is in the diff, at least one
 * lockfile for that ecosystem must also be in the diff, and vice versa.
 *
 * Only checks a lockfile pair if the lockfile itself exists at the project root
 * — that is the signal that this package manager is in use in this repo.
 * Avoids false positives for lock formats the repo doesn't use.
 */
export async function checkLockfileParity(
  changedPaths: readonly string[],
  projectRoot: string,
): Promise<readonly TrustFinding[]> {
  const findings: TrustFinding[] = [];
  const changed = new Set(changedPaths);

  for (const [manifest, lockfile] of LOCKFILE_PAIRS) {
    const manifestInDiff = changed.has(manifest);
    const lockfileInDiff = changed.has(lockfile);

    // Only check parity if the lockfile actually exists at the project root.
    // That presence is the signal that this package manager is in use.
    const lockfileExists = await Bun.file(join(projectRoot, lockfile)).exists();
    if (!lockfileExists) {
      continue;
    }

    if (manifestInDiff && !lockfileInDiff) {
      findings.push({
        check: "lockfile-parity",
        severity: "error",
        paths: [manifest],
        details: `${manifest} changed but ${lockfile} is not in the diff.`,
      });
    } else if (lockfileInDiff && !manifestInDiff) {
      findings.push({
        check: "lockfile-parity",
        severity: "error",
        paths: [lockfile],
        details: `${lockfile} changed but ${manifest} is not in the diff.`,
      });
    }
  }

  return findings;
}
