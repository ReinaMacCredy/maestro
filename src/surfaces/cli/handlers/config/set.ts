/**
 * maestro config-set -- set a settings value by dot-notation key.
 * Writes to .maestro/settings.json (project) by default.
 */

import * as path from 'path';
import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { ensureDir, writeJsonAtomic, readJson } from '../../../../infra/utils/fs-io.ts';
import { setNestedValue } from '../../../../infra/utils/object-utils.ts';
import { homedir } from 'os';

export default defineCommand({
  meta: { name: 'config-set', description: 'Set a settings value\n\nExamples:\n  maestro config-set --key tasks.backend --value br\n  maestro config-set --key dcp.enabled --value true --global' },
  args: {
    key: {
      type: 'string',
      description: 'Settings key with dot notation (e.g. tasks.backend, dcp.enabled, toolbox.deny)',
      required: true,
    },
    value: {
      type: 'string',
      description: 'Value to set (JSON for objects/arrays, plain string otherwise)',
      required: true,
    },
    global: {
      type: 'boolean',
      description: 'Write to global ~/.maestro/settings.json instead of project',
      default: false,
    },
  },
  async run({ args }) {
    try {
      // Parse value
      let parsed: unknown;
      try {
        parsed = JSON.parse(args.value);
      } catch {
        parsed = args.value;
      }

      // Determine target file
      const services = getServices();
      const settingsPath = args.global
        ? path.join(homedir(), '.maestro', 'settings.json')
        : path.join(services.directory, '.maestro', 'settings.json');

      // Read existing, apply change, write back
      const existing = readJson<Record<string, unknown>>(settingsPath) ?? {};
      setNestedValue(existing, args.key, parsed);
      ensureDir(path.dirname(settingsPath));
      writeJsonAtomic(settingsPath, existing);

      // Invalidate settings cache
      services.settingsPort.invalidate();

      output(
        { key: args.key, value: parsed, path: settingsPath },
        () => `[ok] settings '${args.key}' set in ${args.global ? 'global' : 'project'} settings`,
      );
    } catch (err) {
      handleCommandError('config-set', err);
    }
  },
});
