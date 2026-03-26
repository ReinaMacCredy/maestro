/**
 * maestro task-unblock -- unblock a blocked task.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../../infra/utils/resolve.ts';

export default defineCommand({
  meta: { name: 'task-unblock', description: 'Unblock a blocked task\n\nExamples:\n  maestro task-unblock --feature my-feat --task 01-setup --decision "Key obtained"\n  maestro task-unblock --task 01-setup --decision "Resolved" --json' },
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
