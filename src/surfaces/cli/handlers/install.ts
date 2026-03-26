/**
 * maestro install -- install maestro integration for the current host platform.
 *
 * Platform detection priority:
 *   1. CLAUDE_PROJECT_DIR / CLAUDE_SESSION_ID  --> Claude Code
 *   2. CODEX_CI / CODEX_THREAD_ID              --> Codex CLI
 *   3. Fallback                                --> manual setup guidance
 *
 * For Claude Code: print guidance to run `/plugin install maestro`.
 * For Codex: write skills from the embedded registry to .codex/skills/ in cwd.
 * For other: print manual setup instructions.
 */

import { defineCommand } from 'citty';
import { output } from '../../../infra/utils/output.ts';
import { handleCommandError } from '../../../domain/errors.ts';
import { ensureDir, writeAtomic } from '../../../infra/utils/fs-io.ts';
import { BUILTIN_SKILLS } from '../../../app/skills/registry.ts';
import * as path from 'path';

import { detectHost, type HostType } from '../../../infra/utils/host-detect.ts';

interface InstalledSkill {
  slug: string;
  files: string[];
}

/**
 * Write embedded built-in skills to .codex/skills/<slug>/SKILL.md (and any
 * reference files) in the given project root. Uses the registry embedded at
 * build time -- no filesystem dependency on the source tree.
 */
function installCodexSkills(projectRoot: string): InstalledSkill[] {
  const destRoot = path.join(projectRoot, '.codex', 'skills');
  ensureDir(destRoot);

  const installed: InstalledSkill[] = [];

  for (const [slug, skill] of Object.entries(BUILTIN_SKILLS)) {
    const skillDir = path.join(destRoot, slug);
    ensureDir(skillDir);

    const files: string[] = [];

    // Write SKILL.md
    const skillMdPath = path.join(skillDir, 'SKILL.md');
    writeAtomic(skillMdPath, skill.content);
    files.push('SKILL.md');

    // Write reference files if present
    if (skill.references && Object.keys(skill.references).length > 0) {
      const refDir = path.join(skillDir, 'reference');
      ensureDir(refDir);
      for (const [refName, refContent] of Object.entries(skill.references)) {
        const refPath = path.join(refDir, refName);
        // Reference files may be nested (e.g. "subdir/file.md") -- ensure parent
        ensureDir(path.dirname(refPath));
        writeAtomic(refPath, refContent);
        files.push(path.join('reference', refName));
      }
    }

    installed.push({ slug, files });
  }

  return installed;
}

interface InstallResult {
  platform: HostType;
  projectRoot?: string;
  skills?: InstalledSkill[];
  message: string;
}

export default defineCommand({
  meta: { name: 'install', description: 'Install maestro integration for the current host platform\n\nExamples:\n  maestro install\n  maestro install --json' },
  args: {},
  async run() {
    try {
      const platform = detectHost();
      const projectRoot = process.cwd();

      let result: InstallResult;

      if (platform === 'claude-code') {
        result = {
          platform,
          message: [
            'Claude Code detected.',
            '',
            'To install the maestro plugin, run this slash command inside Claude Code:',
            '',
            '  /plugin install maestro',
            '',
            'This registers maestro as a Claude Code plugin and exposes the MCP tools.',
          ].join('\n'),
        };

      } else if (platform === 'codex') {
        const skills = installCodexSkills(projectRoot);
        const totalFiles = skills.reduce((n, s) => n + s.files.length, 0);

        result = {
          platform,
          projectRoot,
          skills,
          message: [
            `[ok] Codex detected. Installed ${skills.length} skills (${totalFiles} files) to .codex/skills/`,
            '',
            ...skills.map(s => `  ${s.slug}  (${s.files.length} files)`),
          ].join('\n'),
        };

      } else {
        result = {
          platform,
          message: [
            'Platform not auto-detected (no CLAUDE_PROJECT_DIR, CODEX_CI, or CODEX_THREAD_ID).',
            '',
            'Manual setup options:',
            '',
            '  Claude Code:',
            '    Run `/plugin install maestro` inside Claude Code.',
            '',
            '  Codex CLI:',
            '    Set CODEX_CI=1 and re-run `maestro install`, or run it inside a Codex session.',
            '',
            '  Other agents:',
            '    Run `maestro skill-list` to see available skills.',
            '    Use `maestro skill <name>` to print any skill\'s content for manual installation.',
          ].join('\n'),
        };
      }

      output(result, r => r.message);
    } catch (err) {
      handleCommandError('install', err);
    }
  },
});
