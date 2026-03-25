/**
 * maestro task-unblock -- unblock a blocked task.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../../core/resolve.ts';

export default defineCommand({
  meta: { name: 'task-unblock', description: 'Unblock a blocked task' },
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
    decision: {
      type: 'string',
      description: 'Decision or resolution for the blocker',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const featureName = requireFeature(services, args.feature, [
        FEATURE_HINT,
      ]);

      const task = await services.taskPort.unblock(featureName, args.task, args.decision);
      output(task, () => `[ok] task '${args.task}' unblocked`);
    } catch (err) {
      handleCommandError('task-unblock', err);
    }
  },
});
