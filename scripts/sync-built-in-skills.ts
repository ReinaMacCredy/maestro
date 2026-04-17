/**
 * Sync `src/infra/domain/built-in-skill-templates.ts` with `skills/built-in/`.
 *
 * `skills/built-in/` is the single source of truth. This script walks it and
 * emits the embedded TS module the compiled binary needs to bootstrap skills
 * at `maestro init` time (the install-site has no repo checkout).
 *
 * Run `bun scripts/sync-built-in-skills.ts` to regenerate, or
 * `bun scripts/sync-built-in-skills.ts --check` in CI to fail on drift.
 */

import { readdir, readFile, stat } from "node:fs/promises";
import { dirname, join, relative, sep } from "node:path";

const ROOT = join(import.meta.dir, "..");
const SOURCE_DIR = join(ROOT, "skills", "built-in");
const TARGET_FILE = join(ROOT, "src", "infra", "domain", "built-in-skill-templates.ts");

interface SkillFile {
  readonly path: string;
  readonly content: string;
}

interface SkillTemplate {
  readonly name: string;
  readonly files: readonly SkillFile[];
}

async function listFilesRecursive(dir: string): Promise<string[]> {
  const entries = (await readdir(dir, { withFileTypes: true }))
    .sort((left, right) => left.name.localeCompare(right.name));
  const files: string[] = [];
  for (const entry of entries) {
    const absolute = join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...await listFilesRecursive(absolute));
      continue;
    }
    if (entry.isFile()) files.push(absolute);
  }
  return files;
}

async function collectTemplates(): Promise<SkillTemplate[]> {
  const skillDirs = (await readdir(SOURCE_DIR, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort((left, right) => left.localeCompare(right));

  const templates: SkillTemplate[] = [];
  for (const dirName of skillDirs) {
    const skillDir = join(SOURCE_DIR, dirName);
    const name = dirName.replaceAll("%3A", ":");
    const absolutePaths = await listFilesRecursive(skillDir);
    const files: SkillFile[] = [];
    for (const absolute of absolutePaths) {
      const relativePath = relative(skillDir, absolute).split(sep).join("/");
      const content = await readFile(absolute, "utf8");
      files.push({ path: relativePath, content });
    }
    templates.push({ name, files });
  }
  return templates;
}

function renderModule(templates: readonly SkillTemplate[]): string {
  const header = [
    "// Generated from skills/built-in so compiled releases can sync shipped skills.",
    "// Edit the .md files under skills/built-in/ and run `bun scripts/sync-built-in-skills.ts`.",
    "export interface BuiltInSkillFile {",
    "  readonly path: string;",
    "  readonly content: string;",
    "}",
    "",
    "export interface BuiltInSkillTemplate {",
    "  readonly name: string;",
    "  readonly files: readonly BuiltInSkillFile[];",
    "}",
    "",
    "export const BUILT_IN_SKILL_TEMPLATES: readonly BuiltInSkillTemplate[] =",
  ].join("\n");

  const body = JSON.stringify(templates, null, 2);
  return `${header}\n${body};\n`;
}

async function readCurrent(): Promise<string | undefined> {
  try {
    return await readFile(TARGET_FILE, "utf8");
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw err;
  }
}

async function main(): Promise<void> {
  const checkMode = process.argv.includes("--check");

  const sourceStat = await stat(SOURCE_DIR).catch(() => undefined);
  if (!sourceStat?.isDirectory()) {
    console.error(`[!] Source directory not found: ${SOURCE_DIR}`);
    process.exit(1);
  }

  const templates = await collectTemplates();
  const rendered = renderModule(templates);
  const current = await readCurrent();

  if (current === rendered) {
    console.log(`[ok] ${relative(ROOT, TARGET_FILE)} is in sync with skills/built-in/`);
    return;
  }

  if (checkMode) {
    console.error(
      `[!] ${relative(ROOT, TARGET_FILE)} is out of sync with skills/built-in/.`,
    );
    console.error("    Run: bun scripts/sync-built-in-skills.ts");
    process.exit(1);
  }

  await Bun.write(TARGET_FILE, rendered);
  const action = current === undefined ? "created" : "updated";
  console.log(`[ok] ${action} ${relative(ROOT, TARGET_FILE)} from skills/built-in/`);
}

await main();
