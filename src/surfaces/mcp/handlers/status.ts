import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY } from '../annotations.ts';
import { requireFeature } from '../../../infra/utils/resolve.ts';
import { featureParam } from '../params.ts';
import { checkStatus } from '../../../app/workflow/status.ts';
import { detectResearchTools } from '../../../app/workflow/research-tools.ts';
import { derivePipelineStage } from '../../../app/workflow/stages.ts';
import { buildPlaybookWithExternalSkills } from '../../../app/workflow/playbook.ts';
import type { WorkflowEngine } from '../../../app/workflow/engine.ts';

export function registerStatusTools(server: McpServer, thunk: ServicesThunk, engine?: WorkflowEngine): void {
  server.registerTool(
    'maestro_status',
    {
      description: 'Get current feature status including pipeline stage, task progress, and recommended next action. Call at session start to orient, or anytime you need to know where a feature stands.',
      inputSchema: {
        feature: featureParam(),
        verbose: z.boolean().optional().default(false).describe('Include researchTools in response'),
      },
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async (input) => {
      const services = thunk.get();
      const feature = requireFeature(services, input.feature);

      const result = await checkStatus(services, feature);
      const pipelineStage = derivePipelineStage({
        planExists: result.plan.exists,
        planApproved: result.plan.approved,
        taskTotal: result.tasks.total,
        taskDone: result.tasks.done,
        contextCount: result.context.count,
      });
      const researchTools = input.verbose ? detectResearchTools(services.directory) : undefined;

      const playbook = buildPlaybookWithExternalSkills(pipelineStage, services.directory, services.workflowRegistry, services.toolbox);

      // Strip heavy fields for MCP -- agents use task_list / plan_read for details
      const { items: _items, ...tasksSummary } = result.tasks;
      const { comments: _comments, ...planSummary } = result.plan;

      const recommendation = engine?.getRecommendation(pipelineStage, result, services.toolbox);

      return respond({
        ...result,
        plan: planSummary,
        tasks: tasksSummary,
        pipelineStage,
        ...(input.verbose && { researchTools }),
        playbook,
        ...(recommendation && { recommendation }),
        skills: { recommended: playbook.skills }, // backward compat
      });
    }),
  );
}
