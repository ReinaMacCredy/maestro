/**
 * Factory for task commands that share identical logic.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../services.ts';
import { output, renderStatusLine } from '../../../infra/utils/output.ts';
import { MaestroError, handleCommandError } from '../../../domain/errors.ts';
import { readStdinText } from '../../../infra/utils/stdin.ts';
import type { TaskInfo } from '../../../domain/types.ts';

export function makeInfoCommand() {
  return defineCommand({
    meta: { name: 'task-info', description: 'Show task details\n\nExamples:\n  maestro task-info --feature my-feat --task 01-setup\n  maestro task-info --feature my-feat --task 01-setup --json' },
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
    meta: { name: `task-${docType}-read`, description: `Read task ${docType}\n\nExamples:\n  maestro task-${docType}-read --feature my-feat --task 01-setup\n  maestro task-${docType}-read --feature my-feat --task 01-setup --json` },
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
    meta: { name: `task-${docType}-write`, description: `Write task ${docType}\n\nExamples:\n  maestro task-${docType}-write --feature my-feat --task 01-setup --content "..."\n  echo "content" | maestro task-${docType}-write --feature my-feat --task 01-setup --stdin` },
    args: {
      feature: { type: 'string' as const, description: 'Feature name', required: true },
      task: { type: 'string' as const, description: 'Task ID', required: true },
      content: { type: 'string' as const, description: `Task ${docType} content (or use --stdin)` },
      stdin: { type: 'boolean' as const, description: 'Read content from stdin', default: false },
    },
    async run({ args }) {
      try {
        const { taskPort } = getServices();
        let rawContent = args.content;
        if (!rawContent && args.stdin) {
          rawContent = await readStdinText();
        }
        if (!rawContent) {
          throw new MaestroError(`No ${docType} content provided`, [
            `Pass --content "..." or --stdin`,
          ]);
        }
        // Unescape literal \n from CLI args to actual newlines
        const content = rawContent.replace(/\\n/g, '\n');
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
