import type { PlanPort } from '../../domain/ports/plan.ts';
import type { FeaturePort } from '../../domain/ports/feature.ts';
import type { TaskPort } from '../../domain/ports/task.ts';
import type { MemoryPort } from '../../domain/ports/memory.ts';
import { MaestroError } from '../../domain/errors.ts';
import { PLAN_SCAFFOLD_TEMPLATE } from '../../templates/plan-scaffold.ts';
import { queryHistoricalContext, type HistoricalPitfall } from '../dcp/historical.ts';

/** Matches task headings at any sub-heading level: ### N. or #### N. etc. */
const TASK_HEADING_RE = /^#{3,6}\s+\d+\.\s+.+$/gm;

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
  warnings?: string[];
}

export interface WritePlanOpts {
  scaffold?: boolean;
  dryRun?: boolean;
}

function generateScaffold(featureName: string): string {
  return PLAN_SCAFFOLD_TEMPLATE.replace('{{featureName}}', featureName);
}

export async function writePlan(
  services: WritePlanServices,
  featureName: string,
  content: string,
  opts?: WritePlanOpts,
): Promise<WritePlanResult> {
  const { planAdapter, featureAdapter } = services;
  featureAdapter.requireActive(featureName);

  const dryRun = opts?.dryRun ?? false;

  if (opts?.scaffold) {
    const template = generateScaffold(featureName);
    const planPath = !dryRun ? planAdapter.write(featureName, template) : '(dry run)';
    const taskHeadings = template.match(TASK_HEADING_RE) || [];
    return { path: planPath, feature: featureName, taskCount: taskHeadings.length, scaffold: true };
  }

  // Validate Discovery section exists and is >= 100 chars
  const discoveryMatch = content.match(/## Discovery\s*\n([\s\S]*?)(?=\n##\s|$)/i);
  if (!discoveryMatch) {
    throw new MaestroError(
      'Plan must include a "## Discovery" section',
      [
        'Add a ## Discovery section documenting research findings',
        'Run `maestro plan-write --scaffold` to generate a valid template',
      ],
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

  // Warn if no tasks found but h2 numbered headings exist (common agent mistake)
  const warnings: string[] = [];
  if (taskHeadings.length === 0) {
    const h2Numbered = content.match(/^##\s+\d+\.\s+.+$/gm);
    if (h2Numbered && h2Numbered.length > 0) {
      warnings.push(
        `Found ${h2Numbered.length} task heading(s) at ## level (h2), but tasks must use ### or #### (h3/h4). ` +
        `## is reserved for section headers (Discovery, Non-Goals, etc.). ` +
        `Change "## 1." to "### 1." and rewrite.`
      );
    } else {
      warnings.push(
        'No task headings found. Use numbered headings like "### 1. Setup database" or "#### 1. Add tests". ' +
        'Run `maestro plan-write --scaffold` to see the expected format.'
      );
    }
  }

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

  let planPath: string;
  if (!dryRun) {
    planPath = planAdapter.write(featureName, content);
    if (wasApproved) {
      featureAdapter.updateStatus(featureName, 'planning');
    }
  } else {
    planPath = '(dry run)';
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

  return { path: planPath, feature: featureName, taskCount: taskHeadings.length, historicalPitfalls, ...(warnings.length > 0 && { warnings }) };
}
