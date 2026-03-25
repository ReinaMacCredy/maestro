/**
 * maestro feature-active -- show or set the active feature.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { renderStatusLine } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';

export default defineCommand({
  meta: { name: 'feature-active', description: 'Show or set active feature' },
  args: {
    name: {
      type: 'positional',
      description: 'Feature name (auto-detected, no-op if provided)',
      required: false,
    },
  },
  async run() {
    try {
      const { featureAdapter } = getServices();
      const active = featureAdapter.getActive();

      output(active, (f) => {
        if (!f) return 'No active feature.';
        return [
          renderStatusLine('Active feature', f.name),
          renderStatusLine('Status', f.status),
        ].join('\n');
      });
    } catch (err) {
      handleCommandError('feature-active', err);
    }
  },
});
