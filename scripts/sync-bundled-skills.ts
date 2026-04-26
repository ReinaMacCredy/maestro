/**
 * Sync `src/infra/domain/bundled-skill-templates.ts` with `skills/bundled/`.
 *
 * `skills/bundled/` is the single source of truth for the maestro skill bundle
 * (maestro-brainstorm, maestro-plan, maestro-task, maestro-mission,
 * maestro-handoff, maestro-setup). This script walks it and emits the embedded
 * TS module the compiled binary needs to install skills into
 * `~/.claude/skills/` and `~/.codex/skills/` via `maestro install` /
 * `maestro update` (the install-site has no repo checkout).
 *
 * Run `bun scripts/sync-bundled-skills.ts` to regenerate, or
 * `bun scripts/sync-bundled-skills.ts --check` in CI to fail on drift.
 */

import { join, relative } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import {
  collectSkillTemplates,
  normalizeLineEndings,
  type SkillTemplate,
} from "./skill-template-source-lib";

const ROOT = join(import.meta.dir, "..");
const SOURCE_DIR = join(ROOT, "skills", "bundled");
const TARGET_FILE = join(ROOT, "src", "infra", "domain", "bundled-skill-templates.ts");

async function collectTemplates(): Promise<readonly SkillTemplate[]> {
  return collectSkillTemplates({
    sourceDir: SOURCE_DIR,
    rootDir: ROOT,
    errorScope: "skills/bundled/",
    includeExecutableMetadata: true,
  });
}

function renderModule(templates: readonly SkillTemplate[]): string {
  const header = [
    "// Generated from skills/bundled so compiled releases can install shipped skills",
    "// into ~/.claude/skills/ and ~/.codex/skills/ via `maestro install` / `maestro update`.",
    "// Edit the files under skills/bundled/ and run `bun scripts/sync-bundled-skills.ts`.",
    "export interface BundledSkillFile {",
    "  readonly path: string;",
    "  readonly content: string;",
    "  readonly executable?: boolean;",
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

export { normalizeLineEndings };

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
