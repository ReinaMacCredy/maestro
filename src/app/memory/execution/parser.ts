/**
 * Parse structured fields from execution memory body.
 * Shared utility -- used by execution-insights, query-historical-context, suggest-doctrine.
 */

import type { FeaturePort } from '../../../domain/ports/feature.ts';
import { parseFrontmatterRich, stripFrontmatter } from '../../../infra/utils/frontmatter.ts';

export interface ParsedExecMemory {
  summary: string;
  filesChanged: number;
  verificationPassed: boolean;
  tags: string[];
  revisionCount: number;
  duration: string | undefined;
}

export function parseExecMemory(content: string): ParsedExecMemory {
  const meta = parseFrontmatterRich(content);
  const body = stripFrontmatter(content);

  const summaryMatch = body.match(/\*\*Summary\*\*:\s*(.+)/);
  const summary = summaryMatch?.[1] ?? '';

  const filesMatch = body.match(/\*\*Files changed\*\*\s*\((\d+)\)/);
  const filesChanged = filesMatch ? parseInt(filesMatch[1], 10) : 0;

  const verificationPassed = body.includes('**Verification**: passed');

  const rawTags = meta?.tags;
  const tags = Array.isArray(rawTags) ? rawTags as string[] : [];

  const revisionMatch = body.match(/\*\*Revisions\*\*:\s*(\d+)/);
  const revisionCount = revisionMatch ? parseInt(revisionMatch[1], 10) : 0;

  const durationMatch = body.match(/\*\*Duration\*\*:\s*(.+)/);
  const duration = durationMatch?.[1]?.trim();

  return { summary, filesChanged, verificationPassed, tags, revisionCount, duration: duration === 'unknown' ? undefined : duration };
}

/**
 * Group memories by their primary tag clusters (excluding "execution" tag).
 * Returns map of cluster key (sorted tags joined with '+') -> memories.
 */
export function groupByTagCluster<T extends { parsed: ParsedExecMemory }>(
  memories: T[],
): Map<string, T[]> {
  const clusters = new Map<string, T[]>();
  for (const mem of memories) {
    const clusterTags = mem.parsed.tags.filter(t => t !== 'execution').sort();
    if (clusterTags.length === 0) continue;
    const key = clusterTags.join('+');
    const existing = clusters.get(key) ?? [];
    existing.push(mem);
    clusters.set(key, existing);
  }
  return clusters;
}

/**
 * Enumerate features sorted by createdAt descending, capped at limit.
 * Shared between query-historical-context and suggest-doctrine.
 */
export function listRecentFeatures(
  featureAdapter: FeaturePort,
  limit: number,
): Array<{ name: string; createdAt: string }> {
  const featureNames = featureAdapter.list();
  const withDate: Array<{ name: string; createdAt: string }> = [];
  for (const name of featureNames) {
    const info = featureAdapter.get(name);
    if (!info) continue;
    withDate.push({ name, createdAt: info.createdAt ?? '1970-01-01T00:00:00Z' });
  }
  withDate.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  return withDate.slice(0, limit);
}
