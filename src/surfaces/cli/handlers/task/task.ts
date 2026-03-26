/**
 * maestro task-block -- mark a task as blocked.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../../infra/utils/resolve.ts';

export default defineCommand({
  meta: { name: 'task-block', description: 'Mark a task as blocked\n\nExamples:\n  maestro task-block --feature my-feat --task 01-setup --reason "Needs API key"\n  maestro task-block --task 01-setup --reason "Blocked by upstream" --json' },
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
