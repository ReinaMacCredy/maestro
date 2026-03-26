/**
 * maestro config-get -- get a config value by key.
 * Reads from settings (v2).
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { MaestroError, handleCommandError } from '../../../../domain/errors.ts';
import { getNestedValue } from '../../../../infra/utils/object-utils.ts';

export default defineCommand({
  meta: { name: 'config-get', description: 'Get a config value\n\nExamples:\n  maestro config-get --key tasks.backend\n  maestro config-get --key dcp.enabled --json' },
  args: {
    key: {
      type: 'string',
      description: 'Config key (e.g. dcp.enabled, tasks.backend, toolbox.deny)',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();

      const settings = services.settingsPort.get();
      const value = getNestedValue(settings, args.key);

      if (value === undefined) {
        throw new MaestroError(`key '${args.key}' not found`);
      }
      output(value, (v) => typeof v === 'object' ? JSON.stringify(v, null, 2) : String(v));
    } catch (err) {
      handleCommandError('config-get', err);
    }
  },
});
