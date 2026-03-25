/**
 * Sync external skill discovery for playbook integration.
 *
 * Deliberately uses readdirSync/readFileSync (not async) because callers
 * (buildPlaybookWithExternalSkills, sessionstart hook) are synchronous.
 * External skill directories are small (0-5 skills typically), sub-millisecond scan.
 *
 * This file MUST NOT import from registry.generated.ts.
 */

import { readdirSync, readFileSync } from 'fs';
import { join } from 'path';
import { parseFrontmatterRich } from '../../infra/utils/frontmatter.ts';
import type { SkillEntry, SkillSource } from './registry.ts';

/** Directories to scan for external skills, in priority order. Shared with registry.ts. */
export const EXTERNAL_SOURCES: ReadonlyArray<{ dir: string; source: SkillSource }> = [
  { dir: 'skills/external', source: 'external' },
  { dir: '.maestro/skills', source: 'maestro' },
  { dir: '.claude/skills', source: 'claude' },
];

/** Per-projectRoot cache -- skills don't change mid-session. */
const _cache = new Map<string, SkillEntry[]>();

/** Clear cache for a project root (used by skill sync). */
export function _clearCache(projectRoot: string): void {
  _cache.delete(projectRoot);
}

/** Discover external skills from project directories (sync). */
export function discoverExternalSkills(projectRoot: string): SkillEntry[] {
  const cached = _cache.get(projectRoot);
  if (cached) return cached;

  const results: SkillEntry[] = [];

  for (const { dir, source } of EXTERNAL_SOURCES) {
    const base = join(projectRoot, dir);
    let entries: import('fs').Dirent[];
    try {
      entries = readdirSync(base, { withFileTypes: true }) as import('fs').Dirent[];
    } catch {
      continue;
    }

    const dirs = (entries as import('fs').Dirent[]).filter(e => e.isDirectory()).map(e => e.name as string).sort();

    for (const slug of dirs) {
      const mdPath = join(base, slug, 'SKILL.md');
      let raw: string;
      try {
        raw = readFileSync(mdPath, 'utf-8');
      } catch {
        continue;
      }

      const fm = parseFrontmatterRich(raw);
      if (!fm?.name || !fm?.description) continue;

      let stage: string[] | undefined;
      if (fm.stage) {
        stage = Array.isArray(fm.stage) ? fm.stage.map(String) : [String(fm.stage)];
      }

      const audience = fm.audience ? String(fm.audience) as 'orchestrator' | 'worker' | 'both' : undefined;

      results.push({
        name: String(fm.name),
        description: String(fm.description),
        source,
        argumentHint: fm['argument-hint'] ? String(fm['argument-hint']) : undefined,
        stage,
        audience,
      });
    }
  }

  _cache.set(projectRoot, results);
  return results;
}

/** Return external skills tagged for a specific pipeline stage (sync). */
export function discoverExternalSkillsByStage(projectRoot: string, stage: string): SkillEntry[] {
  return discoverExternalSkills(projectRoot).filter(s => s.stage?.includes(stage));
}
