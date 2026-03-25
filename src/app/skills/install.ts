/**
 * Skill installation -- copy external skill to .maestro/skills/.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { parseFrontmatterRich } from '../../core/frontmatter.ts';
import { ensureDir } from '../../core/fs-io.ts';
import { MaestroError } from '../../core/errors.ts';

export interface InstallResult {
  name: string;
  path: string;
}

/**
 * Install a skill from a source directory into .maestro/skills/.
 * Validates SKILL.md frontmatter (requires name + description).
 */
export function installSkill(source: string, projectRoot: string): InstallResult {
  const skillMd = path.join(source, 'SKILL.md');
  if (!fs.existsSync(skillMd)) {
    throw new MaestroError(`No SKILL.md found at ${source}`, ['Skill directories must contain a SKILL.md with name/description frontmatter.']);
  }

  const raw = fs.readFileSync(skillMd, 'utf-8');
  const fm = parseFrontmatterRich(raw);
  if (!fm?.name || !fm?.description) {
    throw new MaestroError('SKILL.md missing required frontmatter: name and description');
  }

  const name = String(fm.name);
  const slug = name.toLowerCase().replace(/[^a-z0-9-]/g, '-').replace(/-+/g, '-');
  const targetDir = path.join(projectRoot, '.maestro', 'skills', slug);
  ensureDir(targetDir);

  // Copy all files from source to target
  const entries = fs.readdirSync(source, { withFileTypes: true });
  for (const entry of entries) {
    const srcPath = path.join(source, entry.name);
    const dstPath = path.join(targetDir, entry.name);
    if (entry.isFile()) {
      fs.copyFileSync(srcPath, dstPath);
    } else if (entry.isDirectory()) {
      fs.cpSync(srcPath, dstPath, { recursive: true });
    }
  }

  return { name, path: targetDir };
}
