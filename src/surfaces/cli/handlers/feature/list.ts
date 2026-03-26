/**
 * maestro feature-list -- list all features.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { renderTable } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';

export default defineCommand({
  meta: { name: 'feature-list', description: 'List all features\n\nExamples:\n  maestro feature-list\n  maestro feature-list --json' },
  args: {},
  async run() {
    try {
      const { featureAdapter } = getServices();
      const names = featureAdapter.list();

      const features = names
        .map((name) => featureAdapter.get(name))
        .filter((f) => f !== null);

      output(features, (items) => {
        if (items.length === 0) return 'No features found.';
        const rows = items.map((f: { name: string; status: string; createdAt: string }) => [
          f.name,
          f.status,
          f.createdAt,
        ]);
        return renderTable(['Name', 'Status', 'Created'], rows);
      });
    } catch (err) {
      handleCommandError('feature-list', err);
    }
  },
});
