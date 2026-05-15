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

import { join, relative } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import { decodeSkillDirectoryName } from "@/shared/lib/skill-path.js";
import {
  collectSkillTemplates,
  normalizeLineEndings,
  type SkillTemplate,
} from "./skill-template-source-lib";

const ROOT = join(import.meta.dir, "..");
const SOURCE_DIR = join(ROOT, "skills", "built-in");
const TARGET_FILE = join(ROOT, "src", "infra", "domain", "built-in-skill-templates.ts");

async function collectTemplates(): Promise<readonly SkillTemplate[]> {
  return collectSkillTemplates({
    sourceDir: SOURCE_DIR,
    rootDir: ROOT,
    errorScope: "skills/built-in/",
    mapSkillName: decodeSkillDirectoryName,
  });
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

export { normalizeLineEndings };

export async function syncBuiltInSkills(options: { check?: boolean } = {}): Promise<void> {
  const templates = await collectTemplates();
  const rendered = renderModule(templates);
  const current = normalizeLineEndings(await readText(TARGET_FILE));

  if (current === rendered) {
    console.log(`[ok] ${relative(ROOT, TARGET_FILE)} is in sync with skills/built-in/`);
    return;
  }

  if (options.check) {
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

if (import.meta.main) {
  await syncBuiltInSkills({ check: process.argv.includes("--check") });
}
