import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { dirExists, readJson, writeJson } from "@/shared/lib/fs.js";

const MISSIONS_DIR = join(process.cwd(), ".maestro", "missions");

interface FeatureRecord {
  readonly workerType?: unknown;
  readonly agentType?: unknown;
  readonly [key: string]: unknown;
}

async function migrateFile(path: string): Promise<"migrated" | "skipped" | "error"> {
  try {
    const parsed = await readJson<FeatureRecord>(path);
    if (!parsed) return "error";
    if (!("workerType" in parsed)) return "skipped";

    const { workerType, ...rest } = parsed;
    // Idempotent guard: if a file already carries a non-undefined agentType
    // (partial migration, manual edit, re-run after rollback), preserve the
    // existing value and just strip the stale workerType field.
    if ("agentType" in parsed && parsed.agentType !== undefined) {
      await writeJson(path, rest);
      return "skipped";
    }
    await writeJson(path, { ...rest, agentType: workerType });
    return "migrated";
  } catch {
    console.error(`[!] Failed to migrate ${path}`);
    return "error";
  }
}

interface CollectedFeaturePaths {
  readonly paths: string[];
  readonly missionsDirExists: boolean;
}

async function collectFeaturePaths(): Promise<CollectedFeaturePaths> {
  const missionsDirExists = await dirExists(MISSIONS_DIR);
  if (!missionsDirExists) return { paths: [], missionsDirExists: false };

  const missionDirs = await readdir(MISSIONS_DIR);
  const paths: string[] = [];
  for (const missionId of missionDirs) {
    if (missionId.startsWith(".")) continue;
    const featuresDir = join(MISSIONS_DIR, missionId, "features");
    let entries: string[];
    try {
      entries = await readdir(featuresDir);
    } catch {
      continue;
    }
    for (const entry of entries) {
      if (entry.endsWith(".json")) paths.push(join(featuresDir, entry));
    }
  }
  return { paths, missionsDirExists: true };
}

async function main(): Promise<void> {
  const { paths, missionsDirExists } = await collectFeaturePaths();
  if (paths.length === 0) {
    if (!missionsDirExists) {
      console.log("[ok] No .maestro/missions directory. Nothing to migrate.");
    } else {
      console.log("[ok] .maestro/missions has no feature JSONs. Nothing to migrate.");
    }
    return;
  }

  const results = await Promise.all(paths.map(migrateFile));
  const migrated = results.filter((r) => r === "migrated").length;
  const skipped = results.filter((r) => r === "skipped").length;
  const errors = results.filter((r) => r === "error").length;

  console.log(`[ok] Migrated ${migrated} files (${skipped} already on new schema)`);
  if (errors > 0) {
    console.error(`[!] ${errors} files failed to migrate`);
    process.exit(1);
  }
}

await main();
