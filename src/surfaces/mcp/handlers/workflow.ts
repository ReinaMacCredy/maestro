/**
 * Workflow MCP tools -- maestro_stage for explicit pipeline stage transitions.
 */

import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, errorResponse, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_MUTATING } from '../annotations.ts';
import { buildPlaybook } from '../../../app/workflow/playbook.ts';
import type { PipelineStage } from '../../../app/workflow/stages.ts';

const ALL_STAGES: PipelineStage[] = ['discovery', 'research', 'planning', 'approval', 'execution', 'done'];

export function registerWorkflowTools(server: McpServer, _thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_stage',
    {
      description:
        'Move a feature forward or backward through pipeline stages. Use when skipping a stage ' +
        '(e.g. jumping past research to planning) or retreating to revisit an earlier stage. ' +
        'Stages: discovery -> research -> planning -> approval -> execution -> done. ' +
        'Actions: jump (requires: target, currentStage), skip (advance one), back (retreat one). ' +
        'Example: maestro_stage({ action: "jump", target: "execution", currentStage: "planning" })',
      annotations: ANNOTATIONS_MUTATING,
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
        return errorResponse({ terminal: false, reason: 'validation', error: `Unknown stage: ${current}. Valid: ${ALL_STAGES.join(', ')}` });
      }

      let targetStage: PipelineStage;

      if (input.action === 'jump') {
        if (!input.target || !ALL_STAGES.includes(input.target as PipelineStage)) {
          return errorResponse({ terminal: false, reason: 'validation', error: `Invalid target: ${input.target}. Valid: ${ALL_STAGES.join(', ')}` });
        }
        targetStage = input.target as PipelineStage;
      } else if (input.action === 'skip') {
        if (currentIdx >= ALL_STAGES.length - 1) {
          return errorResponse({ terminal: false, reason: 'invalid_state', error: 'Already at the last stage (done)' });
        }
        targetStage = ALL_STAGES[currentIdx + 1];
      } else {
        if (currentIdx <= 0) {
          return errorResponse({ terminal: false, reason: 'invalid_state', error: 'Already at the first stage (discovery)' });
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
