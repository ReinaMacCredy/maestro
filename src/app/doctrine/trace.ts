/**
 * Doctrine trace file -- append-only record of which doctrine items
 * were injected for a task across revision cycles.
 *
 * Stored at: <taskPath>/doctrine-trace.json
 */

import * as path from 'path';
import { readJson, writeJsonAtomic } from '../../core/fs-io.ts';
import { getTaskPath } from '../../core/paths.ts';

export interface DoctrineTraceEntry {
  revision: number;
  doctrines: string[];
  injectedAt: string;
}

export interface DoctrineTrace {
  entries: DoctrineTraceEntry[];
}

function getTracePath(projectRoot: string, featureName: string, taskFolder: string): string {
  return path.join(getTaskPath(projectRoot, featureName, taskFolder), 'doctrine-trace.json');
}

/** Append an injection entry to the trace file. Best-effort, never throws. */
export function appendDoctrineTrace(
  projectRoot: string,
  featureName: string,
  taskFolder: string,
  revision: number,
  doctrineNames: string[],
): void {
  if (doctrineNames.length === 0) return;

  try {
    const tracePath = getTracePath(projectRoot, featureName, taskFolder);
    const existing = readJson<DoctrineTrace>(tracePath) ?? { entries: [] };

    // Skip if this revision was already traced (guard against hook re-runs)
    if (existing.entries.some(e => e.revision === revision)) return;

    existing.entries.push({
      revision,
      doctrines: doctrineNames,
      injectedAt: new Date().toISOString(),
    });

    writeJsonAtomic(tracePath, existing);
  } catch {
    // Best-effort -- never block agent spawn
  }
}

/** Read the trace file for a task. Returns null if not found. */
export function readDoctrineTrace(
  projectRoot: string,
  featureName: string,
  taskFolder: string,
): DoctrineTrace | null {
  const tracePath = getTracePath(projectRoot, featureName, taskFolder);
  return readJson<DoctrineTrace>(tracePath);
}

/** Collect all unique doctrine names across all revision entries. */
export function collectDoctrineNames(trace: DoctrineTrace): string[] {
  const names = new Set<string>();
  for (const entry of trace.entries) {
    for (const name of entry.doctrines) names.add(name);
  }
  return [...names];
}
