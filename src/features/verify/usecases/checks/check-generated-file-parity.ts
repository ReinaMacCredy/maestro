import { join } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import type { TrustFinding } from "../../domain/types.js";

// Detects sync:* scripts in package.json. Advisory only: lists the detected
// generators so reviewers can verify outputs were regenerated before the diff.
export async function checkGeneratedFileParity(
  projectRoot: string,
): Promise<readonly TrustFinding[]> {
  const raw = await readText(join(projectRoot, "package.json"));
  if (raw === undefined) {
    return [];
  }

  let scripts: Record<string, unknown> | undefined;
  try {
    const pkg = JSON.parse(raw) as Record<string, unknown>;
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
      details: `detected generators: ${syncKeys.join(", ")}; verify sync scripts were run before this commit.`,
    },
  ];
}
