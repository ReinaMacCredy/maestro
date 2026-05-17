import { z } from "zod";
import { MaestroError } from "@/shared/errors.js";
import { parsePolicyYaml } from "@/shared/lib/yaml.js";
import type { MissionTemplate } from "../domain/template-types.js";
import { MissionTemplateLoadError } from "../domain/template-types.js";

const slugRegex = /^[a-z][a-z0-9-]*[a-z0-9]$/;

const seedTaskSchema = z
  .object({
    title: z.string().min(1),
    slug: z.string().regex(slugRegex, "must be kebab-case"),
  })
  .strict();

const templateSchema = z
  .object({
    name: z.string().regex(slugRegex, "must be kebab-case"),
    description: z.string().min(1),
    seedTasks: z.array(seedTaskSchema).min(1),
  })
  .strict();

export function parseTemplateYaml(
  text: string,
  filePath: string,
  expectedName: string,
): MissionTemplate {
  let raw: Record<string, unknown>;
  try {
    raw = parsePolicyYaml<Record<string, unknown>>(text, filePath);
  } catch (err) {
    if (err instanceof MaestroError) {
      throw new MissionTemplateLoadError(filePath, err.message);
    }
    throw err;
  }
  const parsed = templateSchema.safeParse(raw);
  if (!parsed.success) {
    const issue = parsed.error.issues[0];
    const pathStr = issue.path.length > 0 ? issue.path.join(".") : undefined;
    throw new MissionTemplateLoadError(filePath, issue.message, pathStr);
  }
  if (parsed.data.name !== expectedName) {
    throw new MissionTemplateLoadError(
      filePath,
      `name field '${parsed.data.name}' must match filename stem '${expectedName}'`,
      "name",
    );
  }
  const seen = new Set<string>();
  for (let i = 0; i < parsed.data.seedTasks.length; i += 1) {
    const slug = parsed.data.seedTasks[i].slug;
    if (seen.has(slug)) {
      throw new MissionTemplateLoadError(
        filePath,
        `slug '${slug}' appears more than once`,
        `seedTasks.${i}.slug`,
      );
    }
    seen.add(slug);
  }
  return {
    name: parsed.data.name,
    description: parsed.data.description,
    seedTasks: parsed.data.seedTasks,
    source: "user",
  };
}
