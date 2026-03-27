/**
 * maestro handoff-report -- report completion of a cross-agent handoff.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError, MaestroError } from '../../../../domain/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../../infra/utils/resolve.ts';
import { reportCrossAgentHandoff } from '../../../../app/handoff/crossagent.ts';
import { readStdinText } from '../../../../infra/utils/stdin.ts';
import * as fs from 'fs';

export default defineCommand({
  meta: { name: 'handoff-report', description: 'Report completion of a cross-agent handoff\n\nExamples:\n  maestro handoff-report --content "All tasks implemented and tested" --json\n  maestro handoff-report --feature my-feat --file report.md --json' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (defaults to active feature)',
    },
    content: {
      type: 'string',
      alias: 'summary',
      description: 'Completion summary (or use --file / --stdin)',
    },
    file: {
      type: 'string',
      description: 'Read summary from file',
    },
    stdin: {
      type: 'boolean',
      description: 'Read summary from stdin',
      default: false,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const featureName = requireFeature(services, args.feature, [FEATURE_HINT]);

      // Resolve content via cascade
      let content = args.content;
      if (!content && args.file) {
        content = fs.readFileSync(args.file, 'utf-8');
      }
      if (!content && args.stdin) {
        content = await readStdinText();
      }
      if (!content) {
        throw new MaestroError('No summary provided', [
          'Pass --content "..." or --file path/to/report.md or --stdin',
        ]);
      }

      const result = await reportCrossAgentHandoff(
        {
          featureAdapter: services.featureAdapter,
          planAdapter: services.planAdapter,
          taskPort: services.taskPort,
          memoryAdapter: services.memoryAdapter,
          directory: services.directory,
        },
        featureName,
        content,
      );

      output(result, (r) => {
        let text = `[ok] handoff report filed for '${r.feature}'`;
        text += `\n  completed: ${r.tasksCompleted}, pending: ${r.tasksPending}`;
        text += `\n  report: ${r.reportPath}`;
        return text;
      });
    } catch (err) {
      handleCommandError('handoff-report', err);
    }
  },
});
