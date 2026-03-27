/**
 * maestro plan-comment -- add comment to feature plan.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError, MaestroError } from '../../../../domain/errors.ts';
import { readStdinText } from '../../../../infra/utils/stdin.ts';
import * as fs from 'fs';
import { requireFeature, FEATURE_HINT } from '../../../../infra/utils/resolve.ts';

export default defineCommand({
  meta: { name: 'plan-comment', description: 'Add comment to feature plan\n\nExamples:\n  maestro plan-comment --feature my-feat --body "Consider edge case X"\n  maestro plan-comment --feature my-feat --file comment.md --line 42\n  maestro plan-comment --feature my-feat --body "Needs auth" --line 42' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (defaults to active feature)',
    },
    body: {
      type: 'string',
      alias: 'content',
      description: 'Comment body (or use --content, --file, --stdin)',
    },
    file: {
      type: 'string',
      description: 'Read comment body from file',
    },
    stdin: {
      type: 'boolean',
      description: 'Read comment body from stdin',
      default: false,
    },
    line: {
      type: 'string',
      description: 'Line number to attach comment to',
    },
    author: {
      type: 'string',
      description: 'Comment author',
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const featureName = requireFeature(services, args.feature, [FEATURE_HINT]);
      const { planAdapter } = services;

      let body = args.body;
      if (!body && args.file) {
        body = fs.readFileSync(args.file, 'utf-8');
      }
      if (!body && args.stdin) {
        body = await readStdinText();
      }
      if (!body) {
        throw new MaestroError('No comment body provided', [
          'Pass --body "..." or --file path/to/comment.md or --stdin',
        ]);
      }

      const result = planAdapter.addComment(featureName, {
        body,
        author: args.author ?? 'cli',
        line: args.line ? Number(args.line) : 0,
      });

      output(result, () => '[ok] comment added to plan');
    } catch (err) {
      handleCommandError('plan-comment', err);
    }
  },
});
