import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { pathExists } from "@/shared/lib/fs.js";
import type { MissionTemplate } from "../domain/template-types.js";
import { BUILTIN_TEMPLATES } from "./builtin.js";
import { parseTemplateYaml } from "./yaml-parse.js";
import { readdir } from "node:fs/promises";

const USER_TEMPLATES_DIR = join(".maestro", "templates", "missions");

function userTemplatePath(repoRoot: string, name: string): string {
  return join(repoRoot, USER_TEMPLATES_DIR, `${name}.yaml`);
}

export async function loadTemplate(
  name: string,
  repoRoot: string,
): Promise<MissionTemplate | undefined> {
  const userPath = userTemplatePath(repoRoot, name);
  if (await pathExists(userPath)) {
    const text = await readFile(userPath, "utf8");
    return parseTemplateYaml(text, userPath, name);
  }
  return BUILTIN_TEMPLATES.find((t) => t.name === name);
}

export interface ListedTemplates {
  readonly builtin: readonly MissionTemplate[];
  readonly user: readonly MissionTemplate[];
  readonly overrides: readonly string[];
}

export async function listTemplates(repoRoot: string): Promise<ListedTemplates> {
  const userDir = join(repoRoot, USER_TEMPLATES_DIR);
  if (!(await pathExists(userDir))) {
    return { builtin: BUILTIN_TEMPLATES, user: [], overrides: [] };
  }
  let entries: string[];
  try {
    entries = await readdir(userDir);
  } catch {
    return { builtin: BUILTIN_TEMPLATES, user: [], overrides: [] };
  }
  const user: MissionTemplate[] = [];
  const overrides: string[] = [];
  for (const entry of entries) {
    if (!entry.endsWith(".yaml")) continue;
    const name = entry.slice(0, -".yaml".length);
    const filePath = join(userDir, entry);
    const text = await readFile(filePath, "utf8");
    const tpl = parseTemplateYaml(text, filePath, name);
    user.push(tpl);
    if (BUILTIN_TEMPLATES.some((b) => b.name === name)) overrides.push(name);
  }
  user.sort((a, b) => a.name.localeCompare(b.name));
  return { builtin: BUILTIN_TEMPLATES, user, overrides };
}
