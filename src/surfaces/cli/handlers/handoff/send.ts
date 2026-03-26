/**
 * maestro handoff-send -- send handoff to another agent.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { requireFeature, requireHandoffPort, FEATURE_HINT } from '../../../../infra/utils/resolve.ts';

export default defineCommand({
  meta: { name: 'handoff-send', description: 'Send handoff to another agent\n\nExamples:\n  maestro handoff-send --feature my-feat --task 01-setup\n  maestro handoff-send --feature my-feat --task 01-setup --target-agent worker-1' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (defaults to active feature)',
    },
    task: {
      type: 'string',
      description: 'Task ID (folder name)',
      required: true,
    },
    targetAgent: {
      type: 'string',
      description: 'Target agent identifier',
      alias: 'target-agent',
    },
    context: {
      type: 'string',
      description: 'Critical context to include',
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const handoffPort = requireHandoffPort(services);

      const featureName = requireFeature(services, args.feature, [
        FEATURE_HINT,
      ]);

      const handoff = await handoffPort.buildHandoff(featureName, args.task);
      if (args.context) {
        handoff.criticalContext = args.context;
      }

      const result = await handoffPort.sendHandoff(featureName, handoff, args.targetAgent);

      const data = {
        feature: featureName,
        task: args.task,
        filePath: result.filePath,
        threadId: result.threadId,
        agentMailSent: result.agentMailSent,
      };

      output(data, () => {
        let text = `[ok] handoff sent for '${args.task}'\n  file: ${result.filePath}`;
        if (result.threadId) {
          text += `\n  thread: ${result.threadId}`;
        }
        return text;
      });
    } catch (err) {
      handleCommandError('handoff-send', err);
    }
  },
});
