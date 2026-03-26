/**
 * maestro task-list -- list tasks for a feature.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output, renderTaskTable } from '../../../../infra/utils/output.ts';
import { handleCommandError, MaestroError } from '../../../../domain/errors.ts';
import type { TaskStatusType } from '../../../../domain/types.ts';

const VALID_STATUSES: TaskStatusType[] = ['pending', 'claimed', 'done', 'blocked', 'review', 'revision'];

export default defineCommand({
  meta: { name: 'task-list', description: 'List tasks for a feature\n\nExamples:\n  maestro task-list --feature my-feat\n  maestro task-list --feature my-feat --status pending --json' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
    status: {
      type: 'string',
      description: 'Filter by status',
    },
    all: {
      type: 'boolean',
      description: 'Include all tasks (including done)',
      default: false,
    },
  },
  async run({ args }) {
    try {
      let statusFilter: TaskStatusType | undefined;
      if (args.status) {
        if (!VALID_STATUSES.includes(args.status as TaskStatusType)) {
          throw new MaestroError(
            `Invalid status '${args.status}'`,
            [`Valid values: ${VALID_STATUSES.join(', ')}`],
          );
        }
        statusFilter = args.status as TaskStatusType;
      }

      const { taskPort } = getServices();
      const tasks = await taskPort.list(args.feature, {
        status: statusFilter,
        includeAll: args.all,
      });

      output(tasks, (list) => {
        if (list.length === 0) return 'No tasks found.';
        return renderTaskTable(list as { id: string; name: string; status: string; origin: string }[]);
      });
    } catch (err) {
      handleCommandError('task-list', err);
    }
  },
});
