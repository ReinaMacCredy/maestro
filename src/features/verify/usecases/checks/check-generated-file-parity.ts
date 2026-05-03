import { join } from "node:path";
import type { TrustFinding } from "../../domain/types.js";

/**
 * Detects `sync:*` scripts in package.json and emits an info finding.
 *
 * At L2, this check is advisory only — it does NOT run the generators.
 * Actual regeneration is gated behind a future `--regenerate` flag (L2.5).
 * The finding lists detected generators so reviewers know to verify sync.
 */
export async function checkGeneratedFileParity(
  projectRoot: string,
): Promise<readonly TrustFinding[]> {
  const pkgPath = join(projectRoot, "package.json");
  const file = Bun.file(pkgPath);
  if (!(await file.exists())) {
    return [];
  }

  let scripts: Record<string, unknown> | undefined;
  try {
    const pkg = await file.json() as Record<string, unknown>;
    scripts = typeof pkg.scripts === "object" && pkg.scripts !== null
      ? pkg.scripts as Record<string, unknown>
      : undefined;
  } catch {
    return [];
  }

  if (!scripts) {
    return [];
  }

  const syncKeys = Object.keys(scripts).filter((k) => /^sync:/.test(k));
  if (syncKeys.length === 0) {
    return [];
  }

  return [
    {
      check: "generated-file-parity",
      severity: "info",
      paths: [],
      details: `detected generators: ${syncKeys.join(", ")}; pass --regenerate to run them and verify outputs are in sync (not yet implemented at L2).`,
    },
  ];
}
