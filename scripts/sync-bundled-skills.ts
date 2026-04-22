/**
 * Sync `src/infra/domain/bundled-skill-templates.ts` with `skills/bundled/`.
 *
 * `skills/bundled/` is the single source of truth for the maestro skill bundle
 * (maestro-handoff, maestro-task, maestro-mission, maestro-brainstorm,
 * maestro-plan). This script walks it and emits the embedded TS module the
 * compiled binary needs to install skills into `~/.claude/skills/` and
 * `~/.codex/skills/` via `maestro agent inject` (the install-site has no repo
 * checkout).
 *
 * Run `bun scripts/sync-bundled-skills.ts` to regenerate, or
 * `bun scripts/sync-bundled-skills.ts --check` in CI to fail on drift.
 */

import { readdir, readFile } from "node:fs/promises";
import { join, relative, sep } from "node:path";
import { listFilesRecursive, readText } from "@/shared/lib/fs.js";

const ROOT = join(import.meta.dir, "..");
const SOURCE_DIR = join(ROOT, "skills", "bundled");
const TARGET_FILE = join(ROOT, "src", "infra", "domain", "bundled-skill-templates.ts");

interface SkillFile {
  readonly path: string;
  readonly content: string;
}

interface SkillTemplate {
  readonly name: string;
  readonly files: readonly SkillFile[];
}

async function collectTemplates(): Promise<SkillTemplate[]> {
  const skillDirs = (await readdir(SOURCE_DIR, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort((left, right) => left.localeCompare(right));

  const templates: SkillTemplate[] = [];
  for (const dirName of skillDirs) {
    const skillDir = join(SOURCE_DIR, dirName);
    const absolutePaths = await listFilesRecursive(skillDir);
    const files: SkillFile[] = [];
    for (const absolute of absolutePaths) {
      const relativePath = relative(skillDir, absolute).split(sep).join("/");
      const content = normalizeLineEndings(await readFile(absolute, "utf8")) ?? "";
      files.push({ path: relativePath, content });
    }
    templates.push({ name: dirName, files });
  }
  return templates;
}

function renderModule(templates: readonly SkillTemplate[]): string {
  const header = [
    "// Generated from skills/bundled so compiled releases can install shipped skills",
    "// into ~/.claude/skills/ and ~/.codex/skills/ via `maestro install` / `maestro update`.",
    "// Edit the files under skills/bundled/ and run `bun scripts/sync-bundled-skills.ts`.",
    "export interface BundledSkillFile {",
    "  readonly path: string;",
    "  readonly content: string;",
    "}",
    "",
    "export interface BundledSkillTemplate {",
    "  readonly name: string;",
    "  readonly files: readonly BundledSkillFile[];",
    "}",
    "",
    "export const BUNDLED_SKILL_TEMPLATES: readonly BundledSkillTemplate[] =",
  ].join("\n");

  const body = JSON.stringify(templates, null, 2);
  return `${header}\n${body};\n`;
}

export function normalizeLineEndings(text: string | undefined): string | undefined {
  return text?.replace(/\r\n/g, "\n");
}

export async function syncBundledSkills(options: { check?: boolean } = {}): Promise<void> {
  const templates = await collectTemplates();
  const rendered = renderModule(templates);
  const current = normalizeLineEndings(await readText(TARGET_FILE));

  if (current === rendered) {
    console.log(`[ok] ${relative(ROOT, TARGET_FILE)} is in sync with skills/bundled/`);
    return;
  }

  if (options.check) {
    console.error(
      `[!] ${relative(ROOT, TARGET_FILE)} is out of sync with skills/bundled/.`,
    );
    console.error("    Run: bun scripts/sync-bundled-skills.ts");
    process.exit(1);
  }

  await Bun.write(TARGET_FILE, rendered);
  const action = current === undefined ? "created" : "updated";
  console.log(`[ok] ${action} ${relative(ROOT, TARGET_FILE)} from skills/bundled/`);
}

if (import.meta.main) {
  await syncBundledSkills({ check: process.argv.includes("--check") });
}
