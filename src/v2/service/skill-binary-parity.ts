import { BUNDLED_SKILL_TEMPLATES } from "@/infra/domain/bundled-skill-templates.js";
import { parseYaml } from "@/shared/lib/yaml.js";

export interface SkillBinaryDriftFinding {
  readonly skill: string;
  readonly verb: string;
  readonly status: "missing-in-binary";
}

export interface CheckSkillBinaryParityArgs {
  readonly knownVerbs: ReadonlySet<string>;
}

export interface SkillBinaryParityReport {
  readonly skillsChecked: number;
  readonly findings: readonly SkillBinaryDriftFinding[];
}

const VERB_PATTERN = /`maestro ([a-z][a-z0-9-]*(?:\s+[a-z][a-z0-9-]*){0,3})\b/g;

export function checkSkillBinaryParity(
  args: CheckSkillBinaryParityArgs,
): SkillBinaryParityReport {
  const findings: SkillBinaryDriftFinding[] = [];
  let skillsChecked = 0;
  for (const skill of BUNDLED_SKILL_TEMPLATES) {
    const md = findSkillMd(skill);
    if (!md) continue;
    skillsChecked += 1;
    const skipVerbs = extractParitySkipVerbs(md.content);
    const verbs = extractVerbs(md.content);
    for (const verb of verbs) {
      if (skipVerbs.has(verb)) continue;
      const head = firstSegment(verb);
      if (!head) continue;
      if (!args.knownVerbs.has(head)) {
        findings.push({ skill: skill.name, verb, status: "missing-in-binary" });
      }
    }
  }
  return { skillsChecked, findings };
}

function extractParitySkipVerbs(content: string): ReadonlySet<string> {
  if (!content.startsWith("---")) return new Set();
  const close = content.indexOf("\n---", 3);
  if (close === -1) return new Set();
  const rawFrontmatter = content.slice(3, close).trim();
  let frontmatter: Record<string, unknown>;
  try {
    frontmatter = parseYaml<Record<string, unknown>>(rawFrontmatter) ?? {};
  } catch {
    return new Set();
  }
  const raw = frontmatter["parity-skip-verbs"];
  if (!Array.isArray(raw)) return new Set();
  return new Set(raw.filter((entry): entry is string => typeof entry === "string"));
}

export function renderDriftError(finding: SkillBinaryDriftFinding, binaryVersion: string): string {
  return `Skill expects "maestro ${finding.verb}"; binary v${binaryVersion} does not have it. Run "maestro update" or downgrade the skill bundle.`;
}

function findSkillMd(skill: { readonly files: readonly { readonly path: string; readonly content: string }[] }) {
  return skill.files.find((f) => f.path === "SKILL.md");
}

function extractVerbs(content: string): readonly string[] {
  const verbs = new Set<string>();
  for (const match of content.matchAll(VERB_PATTERN)) {
    const v = match[1]?.trim();
    if (v) verbs.add(v);
  }
  return [...verbs];
}

function firstSegment(verb: string): string | undefined {
  return verb.split(/\s+/)[0];
}
