import type { TaskPort } from '../../tasks/port.ts';
import type { FeaturePort } from '../../features/port.ts';
import type { MemoryPort } from '../../memory/port.ts';
import type { DoctrinePort } from '../../doctrine/port.ts';
import { MaestroError } from '../../core/errors.ts';
import type { FeatureJson } from '../../core/types.ts';
import type { DoctrineSettings } from '../../core/settings.ts';
import { suggestDoctrine, type DoctrineSuggestion } from '../doctrine/suggest.ts';
import { consolidateMemories, type ConsolidationResult } from '../memory/consolidate.ts';

export interface CompleteFeatureServices {
  taskPort: TaskPort;
  featureAdapter: FeaturePort;
  memoryAdapter: MemoryPort;
  doctrinePort?: DoctrinePort;
  doctrineConfig?: DoctrineSettings;
}

export interface CompleteFeatureResult {
  feature: FeatureJson;
  tasksSummary: { total: number; done: number };
  promoted: string[];
  consolidation?: ConsolidationResult;
  doctrineSuggestions?: DoctrineSuggestion[];
}

export async function completeFeature(
  services: CompleteFeatureServices,
  featureName: string,
): Promise<CompleteFeatureResult> {
  const { taskPort, featureAdapter, memoryAdapter } = services;
  const feature = featureAdapter.get(featureName);
  if (!feature) throw new MaestroError(`Feature '${featureName}' not found`);
  if (feature.status === 'completed') throw new MaestroError(`Feature '${featureName}' is already completed`);

  const tasks = await taskPort.list(featureName, { includeAll: true });
  const done = tasks.filter(t => t.status === 'done').length;
  const incomplete = tasks.filter(t => t.status !== 'done');

  if (tasks.length === 0) {
    throw new MaestroError(
      'Cannot complete feature: no tasks exist',
      ['Create and complete tasks before marking the feature as done'],
    );
  }

  if (incomplete.length > 0) {
    const incompleteList = incomplete.map(t => `${t.id} (${t.status})`).join(', ');
    const hints = ['Complete all tasks before completing the feature'];
    if (incomplete.some(t => t.status === 'review')) {
      hints.push('Tasks in review need task_accept or task_reject before completion');
    }
    if (incomplete.some(t => t.status === 'revision')) {
      hints.push('Tasks in revision need to be re-claimed and completed');
    }
    throw new MaestroError(
      `Cannot complete feature: ${incomplete.length} task(s) not done: ${incompleteList}`,
      hints,
    );
  }

  // Consolidate memories: merge duplicates, compress stale, auto-promote qualifying
  let consolidation: ConsolidationResult | undefined;
  try {
    consolidation = consolidateMemories(memoryAdapter, featureName, { autoPromote: true });
  } catch {
    // Best-effort -- never block feature completion
  }

  const updated = featureAdapter.complete(featureName);

  // Suggest doctrine candidates from cross-feature patterns (advisory, never blocking)
  let doctrineSuggestions: DoctrineSuggestion[] | undefined;
  if (services.doctrinePort) {
    try {
      const existing = services.doctrinePort.list({ status: 'active' });
      const result = suggestDoctrine(featureAdapter, memoryAdapter, existing, services.doctrineConfig);
      if (result.suggestions.length > 0) {
        doctrineSuggestions = result.suggestions;
      }
    } catch {
      // Best-effort -- never block feature completion
    }
  }

  return {
    feature: updated,
    tasksSummary: { total: tasks.length, done },
    promoted: consolidation?.promoted ?? [],
    consolidation,
    doctrineSuggestions,
  };
}
