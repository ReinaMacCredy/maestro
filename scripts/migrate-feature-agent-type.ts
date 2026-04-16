import { readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

const MISSIONS_DIR = join(process.cwd(), ".maestro", "missions");

interface FeatureRecord {
  [key: string]: unknown;
}

async function migrateFile(path: string): Promise<"migrated" | "skipped" | "error"> {
  let raw: string;
  try {
    raw = await readFile(path, "utf8");
  } catch {
    return "error";
  }

  let parsed: FeatureRecord;
  try {
    parsed = JSON.parse(raw);
  } catch {
    console.error(`[!] Failed to parse ${path}`);
    return "error";
  }

  if (!("workerType" in parsed)) {
    return "skipped";
  }

  const { workerType, ...rest } = parsed;
  const migrated: FeatureRecord = {};
  for (const key of Object.keys(rest)) {
    migrated[key] = rest[key];
    if (key === "description") {
      migrated.agentType = workerType;
    }
  }
  if (!("agentType" in migrated)) {
    migrated.agentType = workerType;
  }

  await writeFile(path, JSON.stringify(migrated, null, 2) + "\n");
  return "migrated";
}

async function main(): Promise<void> {
  let missionDirs: string[];
  try {
    missionDirs = await readdir(MISSIONS_DIR);
  } catch {
    console.log("[ok] No .maestro/missions directory. Nothing to migrate.");
    return;
  }

  let migrated = 0;
  let skipped = 0;

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
      if (!entry.endsWith(".json")) continue;
      const result = await migrateFile(join(featuresDir, entry));
      if (result === "migrated") migrated++;
      if (result === "skipped") skipped++;
    }
  }

  console.log(`[ok] Migrated ${migrated} files (${skipped} already on new schema)`);
}

await main();
