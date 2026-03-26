/**
 * maestro memory-compile -- compile all memory into a single string.
 * Supports DCP-scored compile (--task) and budget-capped compile (--budget).
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { MaestroError, handleCommandError } from '../../../../domain/errors.ts';
import { selectMemories } from '../../../../app/dcp/selector.ts';
import { resolveDcpConfig } from '../../../../app/dcp/config.ts';
import { estimateTokens } from '../../../../infra/utils/tokens.ts';
import { fitWithinBudget } from '../../../../app/dcp/budget.ts';
import type { MemoryFileWithMeta } from '../../../../domain/types.ts';

function parseBudget(raw: string): number {
  const n = parseInt(raw, 10);
  if (Number.isNaN(n) || n <= 0) {
    throw new MaestroError(`--budget must be a positive integer, got '${raw}'`);
  }
  return n;
}

function formatMemories(memories: MemoryFileWithMeta[]): string {
  return memories.length === 0
    ? ''
    : memories.map(m => `## ${m.name}\n\n${m.bodyContent}`).join('\n\n---\n\n');
}

function requireMemories(memories: MemoryFileWithMeta[], feature: string): asserts memories is [MemoryFileWithMeta, ...MemoryFileWithMeta[]] {
  if (memories.length === 0) {
    throw new MaestroError(`no memory files for feature '${feature}'`);
  }
}

export default defineCommand({
  meta: { name: 'memory-compile', description: 'Compile all memory into single string\n\nExamples:\n  maestro memory-compile --feature my-feat\n  maestro memory-compile --feature my-feat --task 01-setup --json' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
    task: {
      type: 'string',
      description: 'Task folder for DCP-scored filtering',
      required: false,
    },
    budget: {
      type: 'string',
      description: 'Byte budget (default from config)',
      required: false,
    },
  },
  async run({ args }) {
    try {
      const { memoryAdapter, taskPort, featureAdapter, settingsPort } = getServices();

      if (args.task) {
        // DCP-scored compile
        const task = await taskPort.get(args.feature, args.task);
        if (!task) {
          throw new MaestroError(`task '${args.task}' not found in feature '${args.feature}'`);
        }
        const memories = memoryAdapter.listWithMeta(args.feature);
        requireMemories(memories, args.feature);
        const cfg = resolveDcpConfig(settingsPort.get().dcp);
        const budget = args.budget ? parseBudget(args.budget) : cfg.memoryBudgetTokens;
        const featureCreatedAt = featureAdapter.get(args.feature)?.createdAt;
        const selected = selectMemories(
          memories, task, task.planTitle ?? null, budget,
          cfg.relevanceThreshold, featureCreatedAt,
        );
        output(formatMemories(selected.memories), (c) => c);
        return;
      }

      if (args.budget) {
        // Budget-capped compile (no DCP scoring, newest first)
        const memories = memoryAdapter.listWithMeta(args.feature);
        requireMemories(memories, args.feature);
        const budget = parseBudget(args.budget);
        const sorted = [...memories].sort((a, b) =>
          new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime(),
        );
        const included = fitWithinBudget(sorted, m => estimateTokens(m.bodyContent), budget);
        output(formatMemories(included), (c) => c);
        return;
      }

      // Legacy: full dump (backward compat)
      const compiled = memoryAdapter.compile(args.feature);
      if (!compiled) {
        throw new MaestroError(`no memory files for feature '${args.feature}'`);
      }
      output(compiled, (c) => c);
    } catch (err) {
      handleCommandError('memory-compile', err);
    }
  },
});
