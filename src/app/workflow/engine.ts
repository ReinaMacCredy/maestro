/**
 * Workflow engine -- orchestrates recommendations and events.
 * Wraps WorkflowRegistry + recommender + event bus into a unified API.
 */

import type { WorkflowRegistry } from './registry.ts';
import type { ToolboxRegistry } from '../../toolbox/registry.ts';
import type { PipelineStage } from './stages.ts';
import { WorkflowEventBus, type WorkflowEventType } from './events.ts';
import { recommend, type WorkflowRecommendation, type RecommendationContext } from './recommender.ts';

export interface StatusContext {
  plan: { exists: boolean; approved: boolean };
  tasks: { total: number; done: number; pending: number; inProgress: number; review?: number; revision?: number };
  context: { count: number };
}

export class WorkflowEngine {
  constructor(
    private registry: WorkflowRegistry,
    private eventBus: WorkflowEventBus,
  ) {}

  /** Build a contextual recommendation from project status. */
  getRecommendation(
    stage: PipelineStage,
    status: StatusContext,
    toolbox?: ToolboxRegistry,
  ): WorkflowRecommendation {
    const context: RecommendationContext = {
      stage,
      taskReview: status.tasks.review ?? 0,
      taskRevision: status.tasks.revision ?? 0,
      taskPending: status.tasks.pending,
      taskClaimed: status.tasks.inProgress,
      planExists: status.plan.exists,
      planApproved: status.plan.approved,
      memoryCount: status.context.count,
    };
    return recommend(this.registry, stage, context, toolbox);
  }

  /** Emit a workflow event. */
  emit(type: WorkflowEventType, feature?: string, task?: string, metadata?: Record<string, unknown>): void {
    this.eventBus.emit({ type, timestamp: new Date().toISOString(), feature, task, metadata });
  }

  /** Access the event bus for direct listener registration. */
  get events(): WorkflowEventBus {
    return this.eventBus;
  }
}

/** Create a workflow engine with its event bus. */
export function createWorkflowEngine(registry: WorkflowRegistry): { engine: WorkflowEngine; eventBus: WorkflowEventBus } {
  const eventBus = new WorkflowEventBus();
  const engine = new WorkflowEngine(registry, eventBus);
  return { engine, eventBus };
}
