/**
 * maestro task-block -- mark a task as blocked.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../../core/resolve.ts';

export default defineCommand({
  meta: { name: 'task-block', description: 'Mark a task as blocked' },
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
    reason: {
      type: 'string',
      description: 'Why the task is blocked',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const featureName = requireFeature(services, args.feature, [
        FEATURE_HINT,
      ]);

      const task = await services.taskPort.block(featureName, args.task, args.reason);
      output(task, () => `[ok] task '${args.task}' blocked: ${args.reason}`);
    } catch (err) {
      handleCommandError('task-block', err);
    }
  },
});
