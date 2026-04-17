import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { dirExists, readJson, writeJson } from "@/shared/lib/fs.js";
import { migrateLegacyWorkerType } from "@/features/mission/feature/feature-migration.js";

const MISSIONS_DIR = join(process.cwd(), ".maestro", "missions");

async function migrateFile(path: string): Promise<"migrated" | "skipped" | "error"> {
  try {
    const parsed = await readJson<unknown>(path);
    if (!parsed) return "error";

    const { normalized, migrated } = migrateLegacyWorkerType(parsed);
    if (!migrated) return "skipped";

    await writeJson(path, normalized);
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
