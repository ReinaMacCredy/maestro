/**
 * maestro agents-md -- manage AGENTS.md file.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../services.ts';
import { output } from '../../../core/output.ts';
import { handleCommandError, MaestroError } from '../../../core/errors.ts';

type Action = 'init' | 'sync' | 'apply';
const VALID_ACTIONS: Action[] = ['init', 'sync', 'apply'];

function formatInitResult(result: { content: string; existed: boolean }): string {
  const status = result.existed ? 'existing AGENTS.md loaded' : 'new AGENTS.md generated';
  return `[ok] ${status} (${result.content.length} chars)`;
}

function formatSyncResult(result: { proposals: string[]; diff: string }): string {
  if (result.proposals.length === 0) return '[ok] AGENTS.md is up to date -- no proposals';
  const lines = [`[ok] ${result.proposals.length} proposal(s):`];
  for (const p of result.proposals) lines.push(`  --> ${p}`);
  if (result.diff) lines.push('', result.diff);
  return lines.join('\n');
}

function formatApplyResult(result: { path: string; chars: number; isNew: boolean }): string {
  const verb = result.isNew ? 'created' : 'updated';
  return `[ok] ${verb} ${result.path} (${result.chars} chars)`;
}

export default defineCommand({
  meta: { name: 'agents-md', description: 'Manage AGENTS.md file (init, sync, apply)' },
  args: {
    action: {
      type: 'string',
      description: 'Action to perform: init, sync, apply',
      required: true,
    },
    feature: {
      type: 'string',
      description: 'Feature name for sync context',
    },
  },
  async run({ args }) {
    try {
      const action = args.action as Action;
      if (!VALID_ACTIONS.includes(action)) {
        throw new MaestroError(
          `Unknown action: ${args.action}`,
          [`Valid actions: ${VALID_ACTIONS.join(', ')}`],
        );
      }

      const { agentsMdAdapter } = getServices();

      if (action === 'init') {
        const result = await agentsMdAdapter.init();
        output(result, formatInitResult);
      } else if (action === 'sync') {
        const feature = args.feature;
        if (!feature) {
          throw new MaestroError(
            'Feature name required for sync',
            ['Specify --feature <name>'],
          );
        }
        const result = await agentsMdAdapter.sync(feature);
        output(result, formatSyncResult);
      } else {
        // apply -- reads current generated content and writes it
        const initResult = await agentsMdAdapter.init();
        const result = agentsMdAdapter.apply(initResult.content);
        output(result, formatApplyResult);
      }
    } catch (err) {
      handleCommandError('agents-md', err);
    }
  },
});
