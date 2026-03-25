import type { MaestroServices } from '../../services.ts';
import type {
  MaestroVisualType,
  VisualResult,
  PlanGraphData,
  StatusDashboardData,
  MemoryMapData,
  ExecutionTimelineData,
  DoctrineNetworkData,
} from './types.ts';
import { renderPage, writeVisual } from '../../visual/renderer.ts';
import { renderPlanGraph } from '../../visual/templates/plan-graph.ts';
import { renderStatusDashboard } from '../../visual/templates/status-dashboard.ts';
import { renderMemoryMap } from '../../visual/templates/memory-map.ts';
import { renderExecutionTimeline } from '../../visual/templates/execution-timeline.ts';
import { renderDoctrineNetwork } from '../../visual/templates/doctrine-network.ts';
import { checkStatus } from '../workflow/status.ts';
import { executionInsights } from '../workflow/insights.ts';
import { MaestroError } from '../../core/errors.ts';
import { derivePipelineStage } from '../workflow/stages.ts';

// ============================================================================
// Data Gathering
// ============================================================================

async function gatherPlanGraph(feature: string, services: MaestroServices): Promise<PlanGraphData> {
  const tasks = await services.taskPort.list(feature, { includeAll: true });
  const plan = services.planAdapter.read(feature);

  return {
    tasks: tasks.map(t => ({
      id: t.id,
      name: t.name ?? t.id,
      status: t.status,
      dependsOn: t.dependsOn ?? [],
      claimedBy: t.claimedBy,
      summary: t.summary,
    })),
    planContent: plan?.content,
    feature,
  };
}

async function gatherStatusDashboard(feature: string, services: MaestroServices): Promise<StatusDashboardData> {
  const status = await checkStatus(services, feature);

  // Doctrine stats from optional port -- single pass
  const doctrineItems = services.doctrinePort?.list() ?? [];
  let active = 0, deprecated = 0;
  for (const d of doctrineItems) {
    if (d.status === 'active') active++;
    else if (d.status === 'deprecated') deprecated++;
  }

  const featureJson = services.featureAdapter.get(feature);

  const pipelineStage = derivePipelineStage({
    planExists: status.plan.exists,
    planApproved: status.plan.approved,
    taskTotal: status.tasks.total,
    taskDone: status.tasks.done,
    contextCount: status.context.count,
  });

  return {
    feature: {
      name: status.feature.name,
      status: status.feature.status,
      createdAt: featureJson?.createdAt ?? '',
      approvedAt: featureJson?.approvedAt,
      completedAt: featureJson?.completedAt,
    },
    tasks: {
      total: status.tasks.total,
      pending: status.tasks.pending,
      claimed: status.tasks.inProgress,
      done: status.tasks.done,
      blocked: status.blocked.length,
      review: status.tasks.review,
      revision: status.tasks.revision,
    },
    runnable: status.runnable,
    blocked: status.blocked,
    pipelineStage,
    memoryStats: { count: status.context.count, totalBytes: status.context.totalBytes },
    doctrineStats: { total: doctrineItems.length, active, deprecated },
    nextAction: status.nextAction,
  };
}

async function gatherMemoryMap(feature: string, services: MaestroServices): Promise<MemoryMapData> {
  const memories = services.memoryAdapter.listWithMeta(feature);

  return {
    memories: memories.map(m => ({
      name: m.name,
      category: m.metadata.category,
      priority: m.metadata.priority,
      tags: m.metadata.tags ?? [],
      sizeBytes: m.sizeBytes,
      updatedAt: m.updatedAt,
    })),
    feature,
  };
}

async function gatherExecutionTimeline(feature: string, services: MaestroServices): Promise<ExecutionTimelineData> {
  const settings = services.settingsPort.get();
  const result = await executionInsights(
    feature,
    services.taskPort,
    services.memoryAdapter,
    services.doctrinePort,
    settings.doctrine,
  );

  return {
    insights: result.insights,
    knowledgeFlow: result.knowledgeFlow,
    coverage: result.coverage,
    doctrineEffectiveness: result.doctrineEffectiveness,
    feature,
  };
}

async function gatherDoctrineNetwork(feature: string, services: MaestroServices): Promise<DoctrineNetworkData> {
  if (!services.doctrinePort) {
    throw new MaestroError('Doctrine not configured', [
      'Run maestro doctrine-write to add doctrine items, then re-run this visualization.',
    ]);
  }

  return {
    items: services.doctrinePort.list(),
    feature,
  };
}

// ============================================================================
// Template Dispatch
// ============================================================================

export async function visualize(
  type: MaestroVisualType,
  featureName: string,
  services: MaestroServices,
  autoOpen: boolean = true,
): Promise<VisualResult> {
  const title = `${type}: ${featureName}`;
  const generatedAt = new Date().toISOString();

  let output;

  switch (type) {
    case 'plan-graph': {
      const data = await gatherPlanGraph(featureName, services);
      output = renderPlanGraph({ data, title, feature: featureName, generatedAt });
      break;
    }
    case 'status-dashboard': {
      const data = await gatherStatusDashboard(featureName, services);
      output = renderStatusDashboard({ data, title, feature: featureName, generatedAt });
      break;
    }
    case 'memory-map': {
      const data = await gatherMemoryMap(featureName, services);
      output = renderMemoryMap({ data, title, feature: featureName, generatedAt });
      break;
    }
    case 'execution-timeline': {
      const data = await gatherExecutionTimeline(featureName, services);
      output = renderExecutionTimeline({ data, title, feature: featureName, generatedAt });
      break;
    }
    case 'doctrine-network': {
      const data = await gatherDoctrineNetwork(featureName, services);
      output = renderDoctrineNetwork({ data, title, feature: featureName, generatedAt });
      break;
    }
    default: {
      const _exhaustive: never = type;
      throw new MaestroError(`Unknown visualization type: ${type}`);
    }
  }

  const html = renderPage({
    title,
    bodyHtml: output.bodyHtml,
    extraHead: output.extraHead,
    extraScripts: output.extraScripts,
  });

  return writeVisual(type, html, featureName, autoOpen);
}
