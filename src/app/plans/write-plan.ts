import type { PlanPort } from '../../plans/port.ts';
import type { FeaturePort } from '../../features/port.ts';
import type { TaskPort } from '../../tasks/port.ts';
import type { MemoryPort } from '../../memory/port.ts';
import { MaestroError } from '../../core/errors.ts';
import scaffoldTemplate from '../../templates/plan-scaffold.md';
import { queryHistoricalContext, type HistoricalPitfall } from '../dcp/historical.ts';

const TASK_HEADING_RE = /^###\s+\d+\.\s+.+$/gm;

export interface WritePlanServices {
  planAdapter: PlanPort;
  featureAdapter: FeaturePort;
  taskPort?: TaskPort;
  memoryAdapter?: MemoryPort;
}

export interface WritePlanResult {
  path: string;
  feature: string;
  taskCount: number;
  scaffold?: boolean;
  historicalPitfalls?: HistoricalPitfall[];
}

export interface WritePlanOpts {
  scaffold?: boolean;
}

function generateScaffold(featureName: string): string {
  return scaffoldTemplate.replace('{{featureName}}', featureName);
}

export async function writePlan(
  services: WritePlanServices,
  featureName: string,
  content: string,
  opts?: WritePlanOpts,
): Promise<WritePlanResult> {
  const { planAdapter, featureAdapter } = services;
  featureAdapter.requireActive(featureName);

  if (opts?.scaffold) {
    const template = generateScaffold(featureName);
    const planPath = planAdapter.write(featureName, template);
    const taskHeadings = template.match(TASK_HEADING_RE) || [];
    return { path: planPath, feature: featureName, taskCount: taskHeadings.length, scaffold: true };
  }

  // Validate Discovery section exists and is >= 100 chars
  const discoveryMatch = content.match(/## Discovery\s*\n([\s\S]*?)(?=\n##\s|$)/i);
  if (!discoveryMatch) {
    throw new MaestroError(
      'Plan must include a "## Discovery" section',
      ['Add a ## Discovery section documenting research findings']
    );
  }
  const discoveryContent = discoveryMatch[1].trim();
  if (discoveryContent.length < 100) {
    throw new MaestroError(
      `Discovery section too short (${discoveryContent.length} chars, min 100)`,
      ['Add more detail to the ## Discovery section']
    );
  }

  // Count task headings (### N. Task Name)
  const taskHeadings = content.match(TASK_HEADING_RE) || [];

  const wasApproved = planAdapter.isApproved(featureName);
  if (wasApproved && services.taskPort) {
    const tasks = await services.taskPort.list(featureName, { includeAll: true });
    if (tasks.length > 0) {
      throw new MaestroError(
        `Plan is approved with ${tasks.length} task(s). Revoke approval first before overwriting.`,
        [`Run: maestro plan-revoke --feature ${featureName}`],
      );
    }
  }

  const planPath = planAdapter.write(featureName, content);
  if (wasApproved) {
    featureAdapter.updateStatus(featureName, 'planning');
  }

  // Query cross-feature historical context (advisory, never blocking)
  let historicalPitfalls: HistoricalPitfall[] | undefined;
  if (services.memoryAdapter) {
    try {
      const result = queryHistoricalContext(content, featureAdapter, services.memoryAdapter);
      if (result.pitfalls.length > 0) {
        historicalPitfalls = result.pitfalls;
      }
    } catch {
      // Best-effort -- never block plan writing
    }
  }

  return { path: planPath, feature: featureName, taskCount: taskHeadings.length, historicalPitfalls };
}
