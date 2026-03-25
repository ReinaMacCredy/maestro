/**
 * Workflow recommender -- contextual tool recommendations with urgency sorting.
 * Uses WorkflowRegistry metadata to suggest next actions based on project state.
 */

import type { WorkflowRegistry, ToolWorkflowMeta } from './registry.ts';
import type { ToolboxRegistry } from '../../toolbox/registry.ts';
import type { PipelineStage } from './stages.ts';

export interface RecommendationContext {
  stage: PipelineStage;
  taskReview: number;
  taskRevision: number;
  taskPending: number;
  taskClaimed: number;
  planExists: boolean;
  planApproved: boolean;
  memoryCount: number;
}

export interface WorkflowRecommendation {
  /** Tools that should be called now (stage-appropriate, high priority). */
  primary: string[];
  /** Tools available but not the immediate next step. */
  secondary: string[];
  /** Tools needed urgently (review/revision tasks require attention). */
  urgent: string[];
  /** Current pipeline stage. */
  stage: PipelineStage;
  /** Context hints from tool metadata keyed by tool name. */
  contextHints: Record<string, string>;
}

/**
 * Build a contextual recommendation from registry metadata and project state.
 */
export function recommend(
  registry: WorkflowRegistry,
  stage: PipelineStage,
  context: RecommendationContext,
  toolbox?: ToolboxRegistry,
): WorkflowRecommendation {
  const stageTools = registry.getToolsForStage(stage, toolbox);
  const contextHints: Record<string, string> = {};

  // Collect context hints
  for (const toolName of stageTools) {
    const meta = registry.getMeta(toolName);
    if (meta?.contextHint) {
      contextHints[toolName] = meta.contextHint;
    }
  }

  // Build urgent list from task state
  const urgent: string[] = [];
  if (context.taskReview > 0) {
    urgent.push('maestro_task_accept', 'maestro_task_reject');
  }
  if (context.taskRevision > 0) {
    urgent.push('maestro_task_claim');
  }

  // Split stage tools into primary and secondary
  const primary: string[] = [];
  const secondary: string[] = [];

  for (const toolName of stageTools) {
    const meta = registry.getMeta(toolName);
    if (!meta) continue;

    if (meta.category === 'primary') {
      primary.push(toolName);
    } else {
      secondary.push(toolName);
    }
  }

  // Add contextual urgency: stage-specific tools that should be called first
  if (stage === 'approval' && context.planApproved && context.taskPending === 0) {
    if (!urgent.includes('maestro_tasks_sync')) {
      urgent.push('maestro_tasks_sync');
    }
  }

  return { primary, secondary, urgent, stage, contextHints };
}
