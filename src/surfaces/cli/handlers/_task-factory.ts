/**
 * Factory for task commands that share identical logic.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../services.ts';
import { output, renderStatusLine } from '../../../core/output.ts';
import { MaestroError, handleCommandError } from '../../../core/errors.ts';
import type { TaskInfo } from '../../../core/types.ts';

export function makeInfoCommand() {
  return defineCommand({
    meta: { name: 'task-info', description: 'Show task details' },
    args: {
      feature: { type: 'string' as const, description: 'Feature name', required: true },
      task: { type: 'string' as const, description: 'Task ID', required: true },
    },
    async run({ args }) {
      try {
        const { taskPort } = getServices();
        const info = await taskPort.get(args.feature, args.task);
        if (!info) {
          throw new MaestroError(`Task '${args.task}' not found in feature '${args.feature}'`);
        }
        output(info, (t: TaskInfo) =>
          [
            renderStatusLine('ID', t.id),
            renderStatusLine('Name', t.name),
            renderStatusLine('Status', t.status),
            renderStatusLine('Origin', t.origin),
            t.planTitle ? renderStatusLine('Plan title', t.planTitle) : null,
            t.summary ? renderStatusLine('Summary', t.summary) : null,
            t.dependsOn?.length ? renderStatusLine('Depends on', t.dependsOn.join(', ')) : null,
          ].filter(Boolean).join('\n'),
        );
      } catch (err) {
        handleCommandError('task-info', err);
      }
    },
  });
}

export function makeDocReadCommand(docType: 'spec' | 'report') {
  const portMethod = docType === 'spec' ? 'readSpec' as const : 'readReport' as const;
  return defineCommand({
    meta: { name: `task-${docType}-read`, description: `Read task ${docType}` },
    args: {
      feature: { type: 'string' as const, description: 'Feature name', required: true },
      task: { type: 'string' as const, description: 'Task ID', required: true },
    },
    async run({ args }) {
      try {
        const { taskPort } = getServices();
        const content = await taskPort[portMethod](args.feature, args.task);
        if (content === null) {
          throw new MaestroError(`No ${docType} found for task '${args.task}'`);
        }
        output({ content }, () => content);
      } catch (err) {
        handleCommandError(`task-${docType}-read`, err);
      }
    },
  });
}

export function makeDocWriteCommand(docType: 'spec' | 'report') {
  const portMethod = docType === 'spec' ? 'writeSpec' as const : 'writeReport' as const;
  return defineCommand({
    meta: { name: `task-${docType}-write`, description: `Write task ${docType}` },
    args: {
      feature: { type: 'string' as const, description: 'Feature name', required: true },
      task: { type: 'string' as const, description: 'Task ID', required: true },
      content: { type: 'string' as const, description: `Task ${docType} content`, required: true },
    },
    async run({ args }) {
      try {
        const { taskPort } = getServices();
        // Unescape literal \n from CLI args to actual newlines
        const content = args.content.replace(/\\n/g, '\n');
        await taskPort[portMethod](args.feature, args.task, content);
        output({ task: args.task }, () =>
          `[ok] ${docType} written for task '${args.task}'`,
        );
      } catch (err) {
        handleCommandError(`task-${docType}-write`, err);
      }
    },
  });
}
