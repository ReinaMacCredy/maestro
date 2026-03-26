/**
 * maestro search-related -- find sessions related to a file.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output, renderTable } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { requireSearchPort } from '../../../../infra/utils/resolve.ts';

export default defineCommand({
  meta: { name: 'search-related', description: 'Find sessions related to a file\n\nExamples:\n  maestro search-related --file src/app.ts\n  maestro search-related --file src/app.ts --limit 10 --json' },
  args: {
    file: {
      type: 'string',
      description: 'File path to search for',
      required: true,
    },
    limit: {
      type: 'string',
      description: 'Max results (default: 5)',
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const searchPort = requireSearchPort(services);

      const limit = args.limit ? parseInt(args.limit, 10) : undefined;
      const results = await searchPort.findRelatedSessions(args.file, limit);

      output({ results }, () => {
        if (results.length === 0) return 'No sessions found.';
        return renderTable(
          ['Session', 'Agent', 'Match', 'Score'],
          results.map((r) => [r.sessionPath, r.agent, r.matchLine, String(r.score)]),
        );
      });
    } catch (err) {
      handleCommandError('search-related', err);
    }
  },
});
