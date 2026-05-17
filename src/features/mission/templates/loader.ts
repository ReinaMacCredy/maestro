import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import type { MissionTemplate } from "../domain/template-types.js";
import { BUILTIN_TEMPLATES } from "./builtin.js";
import { parseTemplateYaml } from "./yaml-parse.js";

const USER_TEMPLATES_DIR = join(".maestro", "templates", "missions");

function userTemplatePath(repoRoot: string, name: string): string {
  return join(repoRoot, USER_TEMPLATES_DIR, `${name}.yaml`);
}

export async function loadTemplate(
  name: string,
  repoRoot: string,
): Promise<MissionTemplate | undefined> {
  const userPath = userTemplatePath(repoRoot, name);
  const text = await readText(userPath);
  if (text !== undefined) return parseTemplateYaml(text, userPath, name);
  return BUILTIN_TEMPLATES.find((t) => t.name === name);
}

export interface ListedTemplates {
  readonly builtin: readonly MissionTemplate[];
  readonly user: readonly MissionTemplate[];
  readonly overrides: readonly string[];
}

export async function listTemplates(repoRoot: string): Promise<ListedTemplates> {
  const userDir = join(repoRoot, USER_TEMPLATES_DIR);
  let entries: string[];
  try {
    entries = await readdir(userDir);
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") {
      return { builtin: BUILTIN_TEMPLATES, user: [], overrides: [] };
    }
    throw err;
  }
  const yamlEntries = entries.filter((e) => e.endsWith(".yaml"));
  const user = await Promise.all(
    yamlEntries.map(async (entry): Promise<MissionTemplate> => {
      const name = entry.slice(0, -".yaml".length);
      const filePath = join(userDir, entry);
      const text = (await readText(filePath)) ?? "";
      return parseTemplateYaml(text, filePath, name);
    }),
  );
  user.sort((a, b) => a.name.localeCompare(b.name));
  const builtinNames = new Set(BUILTIN_TEMPLATES.map((b) => b.name));
  const overrides = user.filter((t) => builtinNames.has(t.name)).map((t) => t.name);
  return { builtin: BUILTIN_TEMPLATES, user, overrides };
}
