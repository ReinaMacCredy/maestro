/**
 * maestro handoff-send -- send handoff to another agent.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError, MaestroError } from '../../../../domain/errors.ts';
import { requireFeature, requireHandoffPort, FEATURE_HINT } from '../../../../infra/utils/resolve.ts';

export default defineCommand({
  meta: { name: 'handoff-send', description: 'Send handoff to another agent\n\nExamples:\n  maestro handoff-send --content "Auth middleware done" --json\n  maestro handoff-send --task 01-setup --to worker-1 --json\n  maestro handoff-send --to worker-1 --content "Ready for review" --json' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (defaults to active feature)',
    },
    task: {
      type: 'string',
      description: 'Task ID (defaults to last completed task)',
    },
    to: {
      type: 'string',
      description: 'Target agent identifier',
      alias: 'target-agent',
    },
    content: {
      type: 'string',
      description: 'Context message to include',
      alias: 'context',
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const handoffPort = requireHandoffPort(services);

      const featureName = requireFeature(services, args.feature, [
        FEATURE_HINT,
      ]);

      // Auto-detect task: use last completed task if --task not provided
      let taskId = args.task;
      if (!taskId && services.taskPort) {
        const allTasks = await services.taskPort.list(featureName, { includeAll: true });
        const doneTasks = allTasks
          .filter((t) => t.status === 'done' && t.completedAt)
          .sort((a, b) => new Date(b.completedAt!).getTime() - new Date(a.completedAt!).getTime());
        if (doneTasks.length > 0) {
          taskId = doneTasks[0].id;
        }
      }
      if (!taskId) {
        throw new MaestroError('No task specified and no completed tasks found', [
          'Pass --task <id> or complete a task first with maestro task-done',
        ]);
      }

      const handoff = await handoffPort.buildHandoff(featureName, taskId);
      if (args.content) {
        handoff.criticalContext = args.content;
      }

      const result = await handoffPort.sendHandoff(featureName, handoff, args.to);

      const data = {
        feature: featureName,
        task: taskId,
        filePath: result.filePath,
        threadId: result.threadId,
        agentMailSent: result.agentMailSent,
      };

      output(data, () => {
        let text = `[ok] handoff sent for '${taskId}'\n  file: ${result.filePath}`;
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
