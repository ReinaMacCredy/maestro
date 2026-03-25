/**
 * Skill sync -- clear discovery cache and re-scan external skills.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { discoverExternalSkills, EXTERNAL_SOURCES, _clearCache } from './external-discovery.ts';

export interface SyncResult {
  discovered: number;
  cleaned: number;
}

/**
 * Re-scan external skills and clean up broken directories (missing SKILL.md).
 */
export function syncSkills(projectRoot: string): SyncResult {
  let cleaned = 0;

  // Clean up broken skill directories
  for (const { dir } of EXTERNAL_SOURCES) {
    const base = path.join(projectRoot, dir);
    let entries: fs.Dirent[];
    try {
      entries = fs.readdirSync(base, { withFileTypes: true });
    } catch {
      continue;
    }

    for (const entry of entries) {
      if (!entry.isDirectory()) continue;
      const skillMd = path.join(base, entry.name, 'SKILL.md');
      if (!fs.existsSync(skillMd)) {
        try {
          fs.rmSync(path.join(base, entry.name), { recursive: true });
          cleaned++;
        } catch { /* best-effort */ }
      }
    }
  }

  // Clear cache and re-discover
  _clearCache(projectRoot);
  const skills = discoverExternalSkills(projectRoot);

  return { discovered: skills.length, cleaned };
}
