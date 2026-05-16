import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileExists } from "@/shared/lib/fs.js";

export const MIGRATION_FLAG_REL = ".maestro/.migrated-v2.json";
export const MIGRATION_FLAG_VERSION = 1;

export interface MigrationFlag {
  readonly version: number;
  readonly migrated_at: string;
  readonly steps: readonly string[];
  readonly backup_path?: string;
}

export async function readMigrationFlag(
  repoRoot: string,
): Promise<MigrationFlag | undefined> {
  const path = join(repoRoot, MIGRATION_FLAG_REL);
  if (!(await fileExists(path))) return undefined;
  const raw = await readFile(path, "utf8");
  return JSON.parse(raw) as MigrationFlag;
}

export async function writeMigrationFlag(
  repoRoot: string,
  flag: MigrationFlag,
): Promise<void> {
  const path = join(repoRoot, MIGRATION_FLAG_REL);
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(flag, null, 2)}\n`, "utf8");
}
