/**
 * DCP injection trace -- append-only record of which memories
 * were injected for a task across revision cycles.
 *
 * Stored at: <taskPath>/dcp-trace.json
 */

import * as path from 'path';
import { readJson, writeJsonAtomic } from '../../infra/utils/fs-io.ts';
import { getTaskPath } from '../../infra/utils/paths.ts';

export interface DcpTraceEntry {
  revision: number;
  memories: Array<{ name: string; score: number }>;
  injectedAt: string;
}

export interface DcpTrace {
  entries: DcpTraceEntry[];
}

function getTracePath(projectRoot: string, featureName: string, taskFolder: string): string {
  return path.join(getTaskPath(projectRoot, featureName, taskFolder), 'dcp-trace.json');
}

/** Append an injection entry to the trace file. Best-effort, never throws. */
export function appendDcpTrace(
  projectRoot: string,
  featureName: string,
  taskFolder: string,
  revision: number,
  memoryScores: Array<{ name: string; score: number }>,
): void {
  if (memoryScores.length === 0) return;

  try {
    const tracePath = getTracePath(projectRoot, featureName, taskFolder);
    const existing = readJson<DcpTrace>(tracePath) ?? { entries: [] };

    // Skip if this revision was already traced (guard against hook re-runs)
    if (existing.entries.some(e => e.revision === revision)) return;

    existing.entries.push({
      revision,
      memories: memoryScores,
      injectedAt: new Date().toISOString(),
    });

    writeJsonAtomic(tracePath, existing);
  } catch {
    // Best-effort -- never block agent spawn
  }
}

/** Read the trace file for a task. Returns null if not found. */
export function readDcpTrace(
  projectRoot: string,
  featureName: string,
  taskFolder: string,
): DcpTrace | null {
  const tracePath = getTracePath(projectRoot, featureName, taskFolder);
  return readJson<DcpTrace>(tracePath);
}

/** Collect all unique memory names across all revision entries. */
export function collectMemoryNames(trace: DcpTrace): string[] {
  const names = new Set<string>();
  for (const entry of trace.entries) {
    for (const m of entry.memories) names.add(m.name);
  }
  return [...names];
}
