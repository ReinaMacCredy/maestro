/**
 * Workflow MCP tools -- maestro_stage for explicit pipeline stage transitions.
 */

import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { buildPlaybook } from '../../../workflow/playbook.ts';
import type { PipelineStage } from '../../../workflow/stages.ts';

const ALL_STAGES: PipelineStage[] = ['discovery', 'research', 'planning', 'approval', 'execution', 'done'];

export function registerWorkflowTools(server: McpServer, _thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_stage',
    {
      description: 'Navigate pipeline stages: jump to a specific stage, skip forward, or go back.',
      inputSchema: {
        action: z.enum(['jump', 'skip', 'back']).describe('Navigation action'),
        target: z.string().optional().describe('Target stage (required for jump)'),
        currentStage: z.string().describe('Current pipeline stage'),
      },
    },
    withErrorHandling(async (input) => {
      const current = input.currentStage as PipelineStage;
      const currentIdx = ALL_STAGES.indexOf(current);
      if (currentIdx === -1) {
        return respond({ error: `Unknown stage: ${current}. Valid: ${ALL_STAGES.join(', ')}` });
      }

      let targetStage: PipelineStage;

      if (input.action === 'jump') {
        if (!input.target || !ALL_STAGES.includes(input.target as PipelineStage)) {
          return respond({ error: `Invalid target: ${input.target}. Valid: ${ALL_STAGES.join(', ')}` });
        }
        targetStage = input.target as PipelineStage;
      } else if (input.action === 'skip') {
        if (currentIdx >= ALL_STAGES.length - 1) {
          return respond({ error: 'Already at the last stage (done)' });
        }
        targetStage = ALL_STAGES[currentIdx + 1];
      } else {
        if (currentIdx <= 0) {
          return respond({ error: 'Already at the first stage (discovery)' });
        }
        targetStage = ALL_STAGES[currentIdx - 1];
      }

      const playbook = buildPlaybook(targetStage);
      return respond({
        previousStage: current,
        newStage: targetStage,
        playbook,
      });
    }),
  );
}
