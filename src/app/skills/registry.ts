import { BUILTIN_SKILLS, BUILTIN_SKILL_NAMES, type BuiltinSkillName } from './registry.generated.ts';
import { SKILL_ALIASES } from './aliases.ts';
import { readdir, readFile } from 'fs/promises';
import { join } from 'path';
import { parseFrontmatter } from '../../core/frontmatter.ts';

export { BUILTIN_SKILLS, BUILTIN_SKILL_NAMES, type BuiltinSkillName };

export type SkillSource = 'builtin' | 'external' | 'maestro' | 'claude';

export interface SkillEntry {
  name: string;
  description: string;
  source: SkillSource;
  argumentHint?: string;
  stage?: string[];
  audience?: 'orchestrator' | 'worker' | 'both';
}

/** Return built-in skills tagged for a specific pipeline stage. */
export function getBuiltinSkillsByStage(stage: string): SkillEntry[] {
  return BUILTIN_SKILL_NAMES
    .filter(name => BUILTIN_SKILLS[name].stage?.includes(stage))
    .map(name => ({
      name,
      description: BUILTIN_SKILLS[name].description,
      source: 'builtin' as SkillSource,
      argumentHint: BUILTIN_SKILLS[name].argumentHint,
      stage: BUILTIN_SKILLS[name].stage,
      audience: BUILTIN_SKILLS[name].audience as SkillEntry['audience'],
    }));
}

import { EXTERNAL_SOURCES } from './external-discovery.ts';

interface ExternalSkill {
  slug: string;
  description: string;
  content: string;
  source: SkillSource;
  dirPath: string;
  argumentHint?: string;
}

/** Skills superseded by native commands. Loaded content gets a deprecation warning prepended. */
const DEPRECATED_SKILLS = new Map<string, string>([
  ['maestro:symphony-setup', 'Use `maestro symphony install --linear-project <slug>` instead.'],
]);

/** Resolve old skill name to canonical name. Returns { resolved, wasAliased }. */
function resolveAlias(name: string): { resolved: string; wasAliased: boolean } {
  const alias = SKILL_ALIASES[name];
  if (alias) {
    return { resolved: alias, wasAliased: true };
  }
  return { resolved: name, wasAliased: false };
}

/** Per-projectRoot cache -- skills don't change mid-session. */
const _externalCache = new Map<string, ExternalSkill[]>();

/**
 * Discover external skills from repository-local and project-local directories.
 * Expects the same layout as builtin: <dir>/<slug>/SKILL.md with name+description frontmatter.
 * Results are cached per projectRoot for the process lifetime.
 */
async function discoverExternal(projectRoot: string): Promise<ExternalSkill[]> {
  const cached = _externalCache.get(projectRoot);
  if (cached) return cached;
  const results: ExternalSkill[] = [];

  for (const { dir, source } of EXTERNAL_SOURCES) {
    const base = join(projectRoot, dir);
    let entries;
    try {
      entries = await readdir(base, { withFileTypes: true });
    } catch {
      continue; // directory doesn't exist -- skip
    }

    const dirs = entries.filter((e) => e.isDirectory()).map((e) => e.name).sort();

    for (const slug of dirs) {
      const mdPath = join(base, slug, 'SKILL.md');
      let raw: string;
      try {
        raw = await readFile(mdPath, 'utf-8');
      } catch {
        continue;
      }

      const fm = parseFrontmatter(raw);
      if (!fm?.name || !fm?.description) continue;

      const skillDirPath = join(base, slug);
      results.push({
        slug,
        description: fm.description,
        content: raw,
        source,
        dirPath: skillDirPath,
        argumentHint: fm['argument-hint'],
      });
    }
  }

  _externalCache.set(projectRoot, results);
  return results;
}

export async function loadSkill(name: string, basePath?: string): Promise<{ content: string } | { error: string }> {
  const projectRoot = basePath || process.cwd();
  const externals = await discoverExternal(projectRoot);

  // Check external sources first (original name) -- external overrides take priority over aliases.
  const directMatch = externals.find((s) => s.slug === name);
  if (directMatch) {
    return { content: directMatch.content };
  }

  // Resolve aliases with deprecation warning.
  const { resolved: resolvedName, wasAliased } = resolveAlias(name);
  if (wasAliased) {
    console.error(`[maestro] Skill '${name}' has been renamed to '${resolvedName}'. Please update your references.`);

    // Check externals again with resolved name (in case external uses new name).
    const aliasMatch = externals.find((s) => s.slug === resolvedName);
    if (aliasMatch) {
      return { content: aliasMatch.content };
    }
  }

  // Fall back to builtin -- content is embedded at build time, no file I/O needed.
  const builtin = BUILTIN_SKILLS[resolvedName as BuiltinSkillName];
  if (builtin) {
    let content = builtin.content;
    const deprecation = DEPRECATED_SKILLS.get(resolvedName);
    if (deprecation) {
      content = `> [!] DEPRECATED: ${deprecation}\n> This skill will be removed in a future release.\n\n${content}`;
    }
    return { content };
  }

  // Collect all available names for the error message
  const allNames = [
    ...BUILTIN_SKILL_NAMES,
    ...externals.map((s) => s.slug),
  ];
  return { error: `Unknown skill: ${resolvedName}. Available: ${allNames.join(', ')}` };
}

/**
 * Load a specific reference file from a skill's reference/ directory.
 * Works for both built-in (embedded) and external (filesystem) skills.
 */
export async function loadSkillReference(
  name: string,
  refPath: string,
  basePath?: string,
): Promise<{ content: string } | { error: string }> {
  const projectRoot = basePath || process.cwd();
  const externals = await discoverExternal(projectRoot);

  // Check external sources first with original name -- external overrides take priority over aliases.
  const { resolved: resolvedName, wasAliased } = resolveAlias(name);
  const match = externals.find((s) => s.slug === name) ??
    (wasAliased ? externals.find((s) => s.slug === resolvedName) : undefined);
  if (match) {
    const refFilePath = join(match.dirPath, 'reference', refPath);
    try {
      const content = await readFile(refFilePath, 'utf-8');
      return { content };
    } catch {
      return { error: `Reference file '${refPath}' not found in skill '${match.slug}'` };
    }
  }

  // Check builtins -- references are embedded at build time.
  const builtin = BUILTIN_SKILLS[resolvedName as BuiltinSkillName];
  if (builtin) {
    if (builtin.references?.[refPath]) {
      return { content: builtin.references[refPath] };
    }
    const available = builtin.references ? Object.keys(builtin.references) : [];
    if (available.length > 0) {
      return { error: `Reference file '${refPath}' not found in skill '${resolvedName}'. Available: ${available.join(', ')}` };
    }
    return { error: `Skill '${resolvedName}' has no reference files` };
  }

  return { error: `Unknown skill: ${resolvedName}` };
}

export async function listSkills(
  basePath?: string,
  opts?: { audience?: 'orchestrator' | 'worker' | 'both' },
): Promise<Array<SkillEntry>> {
  const projectRoot = basePath || process.cwd();
  const seen = new Set<string>();
  const results: SkillEntry[] = [];

  // External skills first so repo-local overrides replace builtin entries in the listing.
  const externals = await discoverExternal(projectRoot);
  for (const s of externals) {
    if (seen.has(s.slug)) {
      continue;
    }
    seen.add(s.slug);
    results.push({ name: s.slug, description: s.description, source: s.source, argumentHint: s.argumentHint });
  }

  // Builtins last; skip any names already provided by external sources.
  for (const name of BUILTIN_SKILL_NAMES) {
    if (seen.has(name)) {
      continue;
    }
    seen.add(name);
    const { description, argumentHint, stage, audience } = BUILTIN_SKILLS[name];
    results.push({
      name,
      description,
      source: 'builtin',
      argumentHint,
      stage,
      audience: audience as SkillEntry['audience'],
    });
  }

  if (opts?.audience) {
    return results.filter(s => !s.audience || s.audience === opts.audience || s.audience === 'both');
  }

  return results;
}
