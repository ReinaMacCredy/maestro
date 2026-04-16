import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { readJson, writeJson } from "@/shared/lib/fs.js";

const MISSIONS_DIR = join(process.cwd(), ".maestro", "missions");

interface FeatureRecord {
  readonly workerType?: unknown;
  readonly agentType?: unknown;
  readonly [key: string]: unknown;
}

async function migrateFile(path: string): Promise<"migrated" | "skipped" | "error"> {
  const parsed = await readJson<FeatureRecord>(path);
  if (!parsed) return "error";
  if (!("workerType" in parsed)) return "skipped";

  const { workerType, ...rest } = parsed;
  await writeJson(path, { ...rest, agentType: workerType });
  return "migrated";
}

async function collectFeaturePaths(): Promise<string[]> {
  let missionDirs: string[];
  try {
    missionDirs = await readdir(MISSIONS_DIR);
  } catch {
    return [];
  }

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
  return paths;
}

async function main(): Promise<void> {
  const paths = await collectFeaturePaths();
  if (paths.length === 0) {
    console.log("[ok] No .maestro/missions directory. Nothing to migrate.");
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
