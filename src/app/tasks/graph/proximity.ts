/**
 * Dependency-proximity scoring for DCP.
 * Computes how close a source task is to a target task in the downstream
 * direction of the dependency graph. Used to boost execution memories
 * from upstream tasks when scoring for downstream tasks.
 */

import { buildEffectiveDependencies, type TaskWithDeps } from './dependency.ts';
import { EXEC_MEMORY_PREFIX } from '../../memory/execution/writer.ts';

/**
 * Extract the source task folder from an execution memory name.
 * Returns null for non-execution memories.
 */
export function extractSourceTask(memoryName: string): string | null {
  if (!memoryName.startsWith(EXEC_MEMORY_PREFIX)) return null;
  return memoryName.slice(EXEC_MEMORY_PREFIX.length);
}

/**
 * Build a downstream adjacency map from the upstream map.
 * Input:  folder -> [folders it depends ON]  (upstream)
 * Output: folder -> [folders that depend ON it]  (downstream)
 */
export function buildDownstreamMap(tasks: TaskWithDeps[]): Map<string, string[]> {
  const upstreamMap = buildEffectiveDependencies(tasks);
  const downstream = new Map<string, string[]>();

  for (const [folder, deps] of upstreamMap) {
    for (const dep of deps) {
      const existing = downstream.get(dep) ?? [];
      existing.push(folder);
      downstream.set(dep, existing);
    }
  }

  return downstream;
}

/**
 * Score the dependency proximity from a source task to a target task.
 * BFS through a prebuilt downstream adjacency map.
 *
 * Returns:
 *   0.0  -- same task or unreachable
 *   0.35 -- 1 hop (direct downstream)
 *   0.15 -- 2 hops
 *   0.05 -- 3+ hops
 */
export function scoreDependencyProximity(
  sourceTaskFolder: string,
  targetTaskFolder: string,
  downstream: Map<string, string[]>,
): number {
  if (sourceTaskFolder === targetTaskFolder) return 0;

  // BFS from source through downstream edges
  const visited = new Set<string>([sourceTaskFolder]);
  let frontier = [sourceTaskFolder];
  let hops = 0;

  while (frontier.length > 0) {
    hops++;
    const nextFrontier: string[] = [];

    for (const node of frontier) {
      const children = downstream.get(node) ?? [];
      for (const child of children) {
        if (child === targetTaskFolder) {
          if (hops === 1) return 0.35;
          if (hops === 2) return 0.15;
          return 0.05;
        }
        if (!visited.has(child)) {
          visited.add(child);
          nextFrontier.push(child);
        }
      }
    }

    frontier = nextFrontier;
  }

  return 0; // Not reachable
}
