/**
 * maestro handoff-plan -- export feature plan for another agent.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../../infra/utils/resolve.ts';
import { buildCrossAgentHandoff } from '../../../../app/handoff/crossagent.ts';
import { readStdinText } from '../../../../infra/utils/stdin.ts';
import * as fs from 'fs';

export default defineCommand({
  meta: { name: 'handoff-plan', description: 'Export feature plan for another agent\n\nExamples:\n  maestro handoff-plan --to codex --json\n  maestro handoff-plan --feature my-feat --to claude --json\n  maestro handoff-plan --to codex --content "Focus on tests" --json\n  maestro handoff-plan --to codex --file context.md --json' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (defaults to active feature)',
    },
    to: {
      type: 'string',
      description: 'Target agent (e.g., "codex", "claude")',
    },
    content: {
      type: 'string',
      description: 'Additional context to include',
    },
    file: {
      type: 'string',
      description: 'Read additional context from file',
    },
    stdin: {
      type: 'boolean',
      description: 'Read additional context from stdin',
      default: false,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const featureName = requireFeature(services, args.feature, [FEATURE_HINT]);

      // Resolve additional context via cascade
      let additionalContext = args.content;
      if (!additionalContext && args.file) {
        additionalContext = fs.readFileSync(args.file, 'utf-8');
      }
      if (!additionalContext && args.stdin) {
        additionalContext = await readStdinText();
      }

      const result = await buildCrossAgentHandoff(
        {
          featureAdapter: services.featureAdapter,
          planAdapter: services.planAdapter,
          taskPort: services.taskPort,
          memoryAdapter: services.memoryAdapter,
          doctrinePort: services.doctrinePort,
          directory: services.directory,
        },
        featureName,
        {
          toAgent: args.to,
          additionalContext,
        },
      );

      const data = {
        feature: featureName,
        handoffPath: result.handoffPath,
        statePath: result.statePath,
        taskCount: result.document.tasks.length,
        to: args.to ?? null,
      };

      output(data, (d) => {
        let text = `[ok] handoff plan exported for '${d.feature}' (${d.taskCount} tasks)`;
        text += `\n  file: ${d.handoffPath}`;
        if (d.to) text += `\n  to: ${d.to}`;
        return text;
      });
    } catch (err) {
      handleCommandError('handoff-plan', err);
    }
  },
});
