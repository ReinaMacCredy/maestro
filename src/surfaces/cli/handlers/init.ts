/**
 * maestro init -- initialize project for maestro.
 */

import { defineCommand } from 'citty';
import { output } from '../../../core/output.ts';
import { handleCommandError } from '../../../core/errors.ts';
import { getMaestroPath } from '../../../core/paths.ts';
import { ensureDir } from '../../../core/fs-io.ts';
import { findProjectRoot } from '../../../features/detection.ts';
import { resolveTaskBackend } from '../../../core/resolve-backend.ts';
import { FsSettingsAdapter } from '../../../core/settings-adapter.ts';
import * as fs from 'fs';
import * as path from 'path';

export default defineCommand({
  meta: { name: 'init', description: 'Initialize maestro for current project' },
  args: {},
  async run() {
    try {
      const cwd = process.cwd();
      const existing = findProjectRoot(cwd);
      const projectRoot = existing || cwd;

      // Create .maestro/ directory
      const maestroPath = getMaestroPath(projectRoot);
      ensureDir(maestroPath);
      ensureDir(path.join(maestroPath, 'features'));

      // Initialize br if .beads/ doesn't exist
      const beadsPath = path.join(projectRoot, '.beads');
      let brInitialized = false;
      if (!fs.existsSync(beadsPath)) {
        try {
          const proc = Bun.spawn(['br', 'init'], { cwd: projectRoot, stdout: 'pipe', stderr: 'pipe' });
          await proc.exited;
          brInitialized = proc.exitCode === 0;
        } catch {
          // br not installed -- not fatal
        }
      } else {
        brInitialized = true;
      }

      const settingsAdapter = new FsSettingsAdapter(projectRoot);
      const configured = settingsAdapter.get().tasks.backend;
      const resolvedBackend = resolveTaskBackend(configured, projectRoot);

      const result = {
        projectRoot,
        maestroPath,
        brInitialized,
        existing: !!existing,
        taskBackend: resolvedBackend,
        wasAutoDetected: (!configured || configured === 'auto') && resolvedBackend === 'br',
      };

      output(result, (r) => {
        const lines = [
          `[ok] maestro initialized at ${r.projectRoot}`,
          `  .maestro/ ${r.existing ? 'already existed' : 'created'}`,
          `  br: ${r.brInitialized ? 'ready' : 'not available (install br for task tracking)'}`,
          `  task backend: ${r.taskBackend}${r.wasAutoDetected ? ' (auto-detected)' : ''}`,
        ];
        return lines.join('\n');
      });
    } catch (err) {
      handleCommandError('init', err);
    }
  },
});
