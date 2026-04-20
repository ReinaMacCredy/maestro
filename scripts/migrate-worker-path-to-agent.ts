import { rename } from "node:fs/promises";
import { basename, join } from "node:path";
import { dirExists, listDirs } from "@/shared/lib/fs.js";

const MAESTRO_DIR = join(process.cwd(), ".maestro");
const MISSIONS_DIR = join(MAESTRO_DIR, "missions");
const SKILLS_DIR = join(MAESTRO_DIR, "skills");

const LEGACY_WORKER_BASE = "maestro:worker-base";
const AGENT_BASE = "maestro:agent-base";

type Outcome = "migrated" | "skipped" | "error";

async function renameDir(from: string, to: string, label: string): Promise<Outcome> {
  const [fromExists, toExists] = await Promise.all([dirExists(from), dirExists(to)]);
  if (!fromExists) return "skipped";
  // dirExists(to) guard is required on macOS, where fs.rename silently
  // replaces an existing destination dir instead of throwing ENOTEMPTY.
  if (toExists) {
    console.log(`[ok] ${label}: skipped -- target already exists (${to})`);
    return "skipped";
  }
  try {
    await rename(from, to);
    console.log(`[ok] ${label}: ${from} -> ${to}`);
    return "migrated";
  } catch (err) {
    console.error(`[!] ${label}: failed to rename ${from}: ${(err as Error).message}`);
    return "error";
  }
}

async function migrateMissionsWorkersDirs(): Promise<readonly Outcome[]> {
  if (!(await dirExists(MISSIONS_DIR))) {
    console.log("[ok] No .maestro/missions directory. Nothing to migrate.");
    return [];
  }

  const missionDirs = (await listDirs(MISSIONS_DIR))
    .filter((path) => !basename(path).startsWith("."));

  return Promise.all(
    missionDirs.map((missionPath) => renameDir(
      join(missionPath, "workers"),
      join(missionPath, "agents"),
      `mission ${basename(missionPath)}`,
    )),
  );
}

async function migrateSkillsWorkerBaseDir(): Promise<Outcome> {
  if (!(await dirExists(SKILLS_DIR))) return "skipped";
  return renameDir(
    join(SKILLS_DIR, LEGACY_WORKER_BASE),
    join(SKILLS_DIR, AGENT_BASE),
    "skill maestro:worker-base",
  );
}

function tally(outcomes: readonly Outcome[]): Record<Outcome, number> {
  const counts: Record<Outcome, number> = { migrated: 0, skipped: 0, error: 0 };
  for (const outcome of outcomes) counts[outcome] += 1;
  return counts;
}

async function main(): Promise<void> {
  const [missionOutcomes, skillOutcome] = await Promise.all([
    migrateMissionsWorkersDirs(),
    migrateSkillsWorkerBaseDir(),
  ]);

  const counts = tally([...missionOutcomes, skillOutcome]);
  console.log(
    `[ok] Migrated ${counts.migrated} directories (${counts.skipped} skipped or already on new layout)`,
  );
  if (counts.error > 0) {
    console.error(`[!] ${counts.error} renames failed`);
    process.exit(1);
  }
}

await main();
