/**
 * maestro task-done -- mark a task as complete.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../../infra/utils/resolve.ts';
import { writeExecutionMemory } from '../../../../app/memory/execution/writer.ts';

export default defineCommand({
  meta: { name: 'task-done', description: 'Mark a task as done\n\nExamples:\n  maestro task-done --task 01-setup --summary "Implemented auth module"\n  maestro task-done --task 01-setup --summary "Added tests" --json' },
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
    summary: {
      type: 'string',
      description: 'Summary of work completed',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const featureName = requireFeature(services, args.feature, [
        FEATURE_HINT,
      ]);

      const existing = await services.taskPort.get(featureName, args.task);
      if (existing) {
        await writeExecutionMemory({
          memoryAdapter: services.memoryAdapter, featureName,
          taskFolder: args.task, task: existing, summary: args.summary,
          projectRoot: services.directory, verificationReport: null,
        });
      }
      const task = await services.taskPort.done(featureName, args.task, args.summary);
      output(task, () => `[ok] task '${args.task}' marked done`);
    } catch (err) {
      handleCommandError('task-done', err);
    }
  },
});
