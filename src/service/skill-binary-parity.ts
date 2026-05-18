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
    // SKILL.md frontmatter is the per-skill skip list. Reference docs ride on
    // it: any verb the SKILL.md author opted out of must not re-appear in
    // reference/*.md without explicit re-opt-in (handled by listing it again).
    const skipVerbs = extractParitySkipVerbs(md.content);
    const mdFiles = collectSkillVerbSources(skill);
    const seen = new Set<string>();
    for (const file of mdFiles) {
      for (const verb of extractVerbs(file.content)) {
        if (skipVerbs.has(verb)) continue;
        if (seen.has(verb)) continue;
        seen.add(verb);
        const head = firstSegment(verb);
        if (!head) continue;
        // `knownVerbs` carries both leaf names and full paths (see how
        // src/index.ts walks the Commander tree). Validate the full verb path
        // so a skill referencing `maestro setup migrate-v2` after that subverb
        // is removed fails parity instead of slipping through on the top-level
        // `setup` match.
        if (!args.knownVerbs.has(verb)) {
          findings.push({ skill: skill.name, verb, status: "missing-in-binary" });
        }
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

// Verbs cited in `reference/*.md` are install-shipped docs that agents will
// read alongside SKILL.md. They must stay parity-checked, or stale v1 doc
// drift hides behind a check that only watches the top-level skill file.
function collectSkillVerbSources(
  skill: { readonly files: readonly { readonly path: string; readonly content: string }[] },
): readonly { readonly path: string; readonly content: string }[] {
  return skill.files.filter(
    (f) =>
      f.path === "SKILL.md" ||
      (f.path.startsWith("reference/") && f.path.endsWith(".md")),
  );
}

function extractVerbs(content: string): readonly string[] {
  const verbs = new Set<string>();
  for (const match of content.matchAll(VERB_PATTERN)) {
    // VERB_PATTERN's `\s+` between segments matches across markdown soft-wraps
    // (``maestro plan\ncheck``). Normalize internal whitespace so the verb
    // shape matches the Commander tree's single-space form.
    const v = match[1]?.trim().replace(/\s+/g, " ");
    if (v) verbs.add(v);
  }
  return [...verbs];
}

function firstSegment(verb: string): string | undefined {
  return verb.split(/\s+/)[0];
}
