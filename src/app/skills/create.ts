/**
 * Skill scaffolding -- create a new skill with SKILL.md template.
 */

import * as path from 'node:path';
import { ensureDir, writeText } from '../../core/fs-io.ts';
import { MaestroError } from '../../core/errors.ts';
import * as fs from 'node:fs';

export interface CreateResult {
  name: string;
  path: string;
}

/**
 * Scaffold a new skill directory with a SKILL.md template.
 */
export function createSkill(name: string, projectRoot: string, stage?: string): CreateResult {
  const slug = name.toLowerCase().replace(/[^a-z0-9-:]/g, '-').replace(/-+/g, '-');
  const targetDir = path.join(projectRoot, '.maestro', 'skills', slug);

  if (fs.existsSync(path.join(targetDir, 'SKILL.md'))) {
    throw new MaestroError(`Skill '${name}' already exists at ${targetDir}`);
  }

  ensureDir(targetDir);

  const stageField = stage ? `\nstage: ${stage}` : '';
  const template = `---
name: ${name}
description: TODO -- describe what this skill does${stageField}
audience: both
---

# ${name}

TODO -- Add skill content here.

## When to Use

Describe when this skill should be loaded.

## Workflow

Describe the workflow steps this skill guides.
`;

  writeText(path.join(targetDir, 'SKILL.md'), template);
  return { name, path: targetDir };
}
