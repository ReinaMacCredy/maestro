/**
 * maestro toolbox-install -- install a tool from a local path.
 */

import * as fs from 'fs';
import * as path from 'path';
import { defineCommand } from 'citty';
import { output } from '../../../../core/output.ts';
import { MaestroError, handleCommandError } from '../../../../core/errors.ts';
import { loadManifest } from '../../../../toolbox/loader.ts';

export default defineCommand({
  meta: { name: 'toolbox-install', description: 'Install a tool from a local path' },
  args: {
    source: {
      type: 'string',
      description: 'Path to tool directory (must contain manifest.json)',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const sourcePath = path.resolve(args.source);

      if (!fs.existsSync(sourcePath)) {
        throw new MaestroError(`Source path not found: ${sourcePath}`);
      }

      const manifestPath = path.join(sourcePath, 'manifest.json');
      const manifest = loadManifest(manifestPath);
      if (!manifest) {
        throw new MaestroError(
          `No valid manifest.json found at ${manifestPath}`,
          ['manifest.json must have at least "name" and "priority" fields'],
        );
      }

      const targetDir = path.join(import.meta.dir, '../../../toolbox/tools/external', manifest.name);
      if (fs.existsSync(targetDir)) {
        throw new MaestroError(
          `Tool '${manifest.name}' already exists`,
          [`Remove it first with: maestro toolbox-remove ${manifest.name}`],
        );
      }

      // Copy source directory to target
      fs.cpSync(sourcePath, targetDir, { recursive: true });

      output(
        { name: manifest.name, transport: manifest.transport ?? 'unknown', path: targetDir },
        () => `[ok] Installed tool '${manifest.name}' from ${sourcePath}`,
      );
    } catch (err) {
      handleCommandError('toolbox-install', err);
    }
  },
});
