/**
 * Query the execution knowledge graph for a feature.
 * Lists execution memories, computes downstream edges, and calculates coverage.
 */

import type { TaskPort } from '../../tasks/port.ts';
import type { MemoryPort } from '../../memory/port.ts';
import type { DoctrinePort } from '../../doctrine/port.ts';
import { buildDownstreamMap, extractSourceTask, scoreDependencyProximity } from '../tasks/graph/proximity.ts';
import type { TaskWithDeps } from '../tasks/graph/dependency.ts';
import { isExecutionMemory } from '../memory/execution/writer.ts';
import { parseExecMemory } from '../memory/execution/parser.ts';
import { resolveDoctrineConfig } from '../doctrine/config.ts';
import type { DoctrineSettings } from '../../core/settings.ts';

export interface ExecutionInsight {
  sourceTask: string;
  summary: string;
  filesChanged: number;
  verificationPassed: boolean;
  tags: string[];
  downstreamTasks: string[];
}

export interface DoctrineEffectivenessInsight {
  name: string;
  injectionCount: number;
  successRate: number;
  overrideCount: number;
  stale: boolean;
}

export interface ExecutionInsightsResult {
  feature: string;
  insights: ExecutionInsight[];
  coverage: {
    totalTasks: number;
    withExecMemory: number;
    percent: number;
  };
  knowledgeFlow: Array<{ from: string; to: string; proximity: number }>;
  doctrineEffectiveness?: DoctrineEffectivenessInsight[];
}

/** Grace period for never-injected doctrine before marking stale (30 days). */
const NEVER_INJECTED_GRACE_MS = 30 * 24 * 60 * 60 * 1000;

export async function executionInsights(
  featureName: string,
  taskPort: TaskPort,
  memoryAdapter: MemoryPort,
  doctrinePort?: DoctrinePort,
  doctrineConfig?: DoctrineSettings,
): Promise<ExecutionInsightsResult> {
  const allMemories = memoryAdapter.listWithMeta(featureName);
  const execMemories = allMemories.filter(m => isExecutionMemory(m.name));

  const allTasks = await taskPort.list(featureName, { includeAll: true });
  // Bidirectional translation between folder and id
  const folderToId = new Map(allTasks.map(t => [t.folder, t.id ?? t.folder]));
  const idToFolder = new Map(allTasks.map(t => [t.id ?? t.folder, t.folder]));

  // Normalize dependsOn: translate any folder references to ids so the graph is id-consistent
  const taskDeps: TaskWithDeps[] = allTasks.map(t => ({
    id: t.id ?? t.folder,
    folder: t.folder,
    status: t.status,
    dependsOn: (t.dependsOn ?? []).map(dep => folderToId.get(dep) ?? dep),
  }));

  const downstream = buildDownstreamMap(taskDeps);

  const taskFolders = new Set(allTasks.map(t => t.folder));
  const execTaskFolders = new Set<string>();
  const insights: ExecutionInsight[] = [];

  for (const mem of execMemories) {
    const sourceFolder = extractSourceTask(mem.name);
    if (!sourceFolder || !taskFolders.has(sourceFolder)) continue;

    execTaskFolders.add(sourceFolder);
    const parsed = parseExecMemory(mem.content);

    // downstream is keyed by id -- translate folder to id for lookup, then back to folders
    const sourceId = folderToId.get(sourceFolder) ?? sourceFolder;
    const downstreamIds = downstream.get(sourceId) ?? [];
    const downstreamTasks = downstreamIds.map(id => idToFolder.get(id) ?? id);

    insights.push({
      sourceTask: sourceFolder,
      summary: parsed.summary,
      filesChanged: parsed.filesChanged,
      verificationPassed: parsed.verificationPassed,
      tags: parsed.tags,
      downstreamTasks,
    });
  }

  const totalTasks = allTasks.length;
  const withExecMemory = execTaskFolders.size;
  const percent = totalTasks > 0 ? Math.round((withExecMemory / totalTasks) * 100) : 0;

  const knowledgeFlow: Array<{ from: string; to: string; proximity: number }> = [];
  for (const sourceFolder of execTaskFolders) {
    const sourceId = folderToId.get(sourceFolder) ?? sourceFolder;
    for (const targetFolder of taskFolders) {
      if (targetFolder === sourceFolder) continue;
      const targetId = folderToId.get(targetFolder) ?? targetFolder;
      const proximity = scoreDependencyProximity(sourceId, targetId, downstream);
      if (proximity > 0) {
        knowledgeFlow.push({ from: sourceFolder, to: targetFolder, proximity });
      }
    }
  }

  knowledgeFlow.sort((a, b) => b.proximity - a.proximity);

  let doctrineEffectiveness: DoctrineEffectivenessInsight[] | undefined;
  if (doctrinePort) {
    try {
      const cfg = resolveDoctrineConfig(doctrineConfig);
      const now = Date.now();
      const staleMs = cfg.staleThresholdDays * 24 * 60 * 60 * 1000;
      const activeItems = doctrinePort.list({ status: 'active' });

      doctrineEffectiveness = activeItems.map(item => {
        const lastInjected = item.effectiveness.lastInjectedAt
          ? new Date(item.effectiveness.lastInjectedAt).getTime()
          : 0;
        const neverInjected = item.effectiveness.injectionCount === 0;
        const created = new Date(item.createdAt).getTime();
        const stale = neverInjected
          ? (now - created > NEVER_INJECTED_GRACE_MS)
          : (now - lastInjected > staleMs);

        return {
          name: item.name,
          injectionCount: item.effectiveness.injectionCount,
          successRate: item.effectiveness.associatedSuccessRate,
          overrideCount: item.effectiveness.overrideCount,
          stale,
        };
      });
    } catch {
      // Best-effort
    }
  }

  return {
    feature: featureName,
    insights,
    coverage: { totalTasks, withExecMemory, percent },
    knowledgeFlow,
    doctrineEffectiveness,
  };
}
