/**
 * DCP telemetry -- records task outcomes correlated with injected memories.
 * Stored at: .maestro/telemetry.jsonl (project-global, append-only)
 *
 * Read side: buildEffectivenessMap() computes per-memory success scores
 * from accumulated telemetry, used as the 6th DCP scoring signal.
 */

import * as fs from 'fs';
import * as path from 'path';

export type TelemetryOutcome = 'success' | 'revision' | 'blocked';

export interface TelemetryRecord {
  taskId: string;
  featureName: string;
  timestamp: string;
  injectedMemories: string[];
  outcome: TelemetryOutcome;
  revisionCount: number;
  verificationPassed: boolean | null;
}

const TELEMETRY_FILE = 'telemetry.jsonl';
const DEFAULT_MAX_RECORDS = 500;
const THIRTY_DAYS_MS = 30 * 24 * 60 * 60 * 1000;

function getTelemetryPath(projectRoot: string): string {
  return path.join(projectRoot, '.maestro', TELEMETRY_FILE);
}

/** Append a telemetry record. Best-effort, never throws. */
export function recordTelemetry(projectRoot: string, record: TelemetryRecord): void {
  try {
    const filePath = getTelemetryPath(projectRoot);
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    fs.appendFileSync(filePath, JSON.stringify(record) + '\n');
  } catch {
    // Best-effort -- never block task completion
  }
}

/**
 * Load telemetry records from disk.
 * Returns the last `maxRecords` entries (most recent matter most).
 * Returns [] on missing file or parse errors.
 */
export function loadTelemetry(
  projectRoot: string,
  maxRecords: number = DEFAULT_MAX_RECORDS,
): TelemetryRecord[] {
  try {
    const filePath = getTelemetryPath(projectRoot);
    const content = fs.readFileSync(filePath, 'utf-8');
    const lines = content.trim().split('\n').filter(Boolean);

    // Take the tail (most recent)
    const tail = maxRecords > 0 && lines.length > maxRecords
      ? lines.slice(-maxRecords)
      : lines;

    const records: TelemetryRecord[] = [];
    for (const line of tail) {
      try {
        records.push(JSON.parse(line));
      } catch {
        // Skip malformed lines
      }
    }
    return records;
  } catch {
    return [];
  }
}

/**
 * Build a map of memory name -> effectiveness score (0.0-1.0).
 * Memories with fewer than `minSamples` observations are omitted
 * (callers should default to 0.5 neutral for missing entries).
 *
 * Outcome scoring: success=1.0, revision(1)=0.4, revision(2+)=0.1, blocked=0.0
 * Weighting: exponential recency decay with 30-day half-life.
 */
export function buildEffectivenessMap(
  records: TelemetryRecord[],
  minSamples: number = 3,
): Map<string, number> {
  if (records.length === 0) return new Map();

  const now = Date.now();

  // Accumulate per-memory weighted outcomes
  const memoryStats = new Map<string, { weightedSum: number; totalWeight: number; count: number }>();

  for (const record of records) {
    const age = now - new Date(record.timestamp).getTime();
    const recencyWeight = Math.exp(-age / THIRTY_DAYS_MS);

    const outcomeScore =
      record.outcome === 'success' ? 1.0
        : record.outcome === 'blocked' ? 0.0
        : record.revisionCount <= 1 ? 0.4
        : 0.1;

    for (const name of record.injectedMemories) {
      const existing = memoryStats.get(name) ?? { weightedSum: 0, totalWeight: 0, count: 0 };
      existing.weightedSum += outcomeScore * recencyWeight;
      existing.totalWeight += recencyWeight;
      existing.count += 1;
      memoryStats.set(name, existing);
    }
  }

  // Build effectiveness map, filtering by minSamples
  const result = new Map<string, number>();
  for (const [name, stats] of memoryStats) {
    if (stats.count < minSamples) continue;
    result.set(name, stats.totalWeight > 0 ? stats.weightedSum / stats.totalWeight : 0.5);
  }

  return result;
}
