/**
 * Memory consolidation pipeline.
 * Detects duplicates, compresses stale memories, identifies promotion candidates.
 */

import type { MemoryFileWithMeta, MemoryMetadata, MemoryConnection } from '../../core/types.ts';
import type { MemoryPort } from '../../memory/port.ts';
import { extractKeywords, computeSetOverlap } from '../dcp/relevance.ts';

const DUPLICATE_THRESHOLD = 0.8;  // 80% keyword overlap = duplicate
const CONNECTION_THRESHOLD = 0.4; // 40-79% keyword overlap = related
const STALE_DAYS = 90;
const STALE_MIN_PRIORITY = 3;     // only auto-archive low-priority (3-4)
const PROMOTE_MIN_SELECTIONS = 3;
const PROMOTE_MAX_PRIORITY = 1;   // only auto-promote high-priority (0-1)
const PROMOTE_CATEGORIES = new Set(['decision', 'architecture']);

export interface ConsolidationOpts {
  autoPromote?: boolean;
  dryRun?: boolean;
}

export interface ConsolidationResult {
  merged: Array<{ kept: string; removed: string; reason: string }>;
  compressed: string[];
  promoted: string[];
  promotionCandidates: string[];
  connections: Array<{ source: string; target: string; relation: string }>;
  stats: { total: number; afterConsolidation: number };
}

/**
 * Run the consolidation pipeline on a feature's memories.
 */
export function consolidateMemories(
  memoryAdapter: MemoryPort,
  feature: string,
  opts: ConsolidationOpts = {},
): ConsolidationResult {
  const memories = memoryAdapter.listWithMeta(feature);
  if (memories.length === 0) {
    return { merged: [], compressed: [], promoted: [], promotionCandidates: [], connections: [], stats: { total: 0, afterConsolidation: 0 } };
  }

  const result: ConsolidationResult = {
    merged: [],
    compressed: [],
    promoted: [],
    promotionCandidates: [],
    connections: [],
    stats: { total: memories.length, afterConsolidation: memories.length },
  };

  // Build keyword sets for each memory
  const keywordSets = new Map<string, Set<string>>();
  for (const mem of memories) {
    keywordSets.set(mem.name, extractKeywords(mem.bodyContent));
  }

  const removed = new Set<string>();
  const pendingConnections: Array<{ source: string; target: string }> = [];

  // Step 1: Pairwise overlap -- classify as duplicate (>=0.8), related (>=0.4), or skip
  for (let i = 0; i < memories.length; i++) {
    if (removed.has(memories[i].name)) continue;
    for (let j = i + 1; j < memories.length; j++) {
      if (removed.has(memories[j].name)) continue;

      const a = memories[i];
      const b = memories[j];
      const overlap = computeSetOverlap(keywordSets.get(a.name)!, keywordSets.get(b.name)!, 'jaccard');

      if (overlap >= DUPLICATE_THRESHOLD && a.metadata.category === b.metadata.category) {
        const keepA = (a.metadata.priority ?? 2) <= (b.metadata.priority ?? 2);
        const kept = keepA ? a : b;
        const dup = keepA ? b : a;
        if (!opts.dryRun) {
          memoryAdapter.delete(feature, dup.name);
        }
        removed.add(dup.name);
        result.merged.push({ kept: kept.name, removed: dup.name, reason: `${Math.round(overlap * 100)}% keyword overlap` });
      } else if (overlap >= CONNECTION_THRESHOLD) {
        pendingConnections.push({ source: a.name, target: b.name });
      }
    }
  }

  // Step 2: Detect stale memories (>90 days, priority 3+, 0 selections)
  const now = Date.now();
  const staleCutoff = now - STALE_DAYS * 24 * 60 * 60 * 1000;
  for (const mem of memories) {
    if (removed.has(mem.name)) continue;

    const priority = mem.metadata.priority ?? 2;
    const selections = mem.metadata.selectionCount ?? 0;
    const mtime = new Date(mem.updatedAt).getTime();

    if (mtime < staleCutoff && priority >= STALE_MIN_PRIORITY && selections === 0) {
      if (!opts.dryRun) {
        // Compress: keep first paragraph + metadata only
        const compressed = compressMemory(mem.bodyContent);
        memoryAdapter.write(feature, mem.name, compressed);
      }
      result.compressed.push(mem.name);
    }
  }

  // Step 3: Detect promotion candidates (priority 0-1, decision/architecture, 3+ selections)
  for (const mem of memories) {
    if (removed.has(mem.name)) continue;

    const priority = mem.metadata.priority ?? 2;
    const selections = mem.metadata.selectionCount ?? 0;
    const category = mem.metadata.category;

    if (priority <= PROMOTE_MAX_PRIORITY && PROMOTE_CATEGORIES.has(category ?? '') && selections >= PROMOTE_MIN_SELECTIONS) {
      result.promotionCandidates.push(mem.name);

      if (opts.autoPromote && !opts.dryRun) {
        const content = memoryAdapter.read(feature, mem.name);
        if (content) {
          memoryAdapter.writeGlobal(mem.name, content);
          result.promoted.push(mem.name);
        }
      }
    }
  }

  // Step 4: Record connections detected in Step 1 (skip pairs where either was removed)
  for (const { source, target } of pendingConnections) {
    if (removed.has(source) || removed.has(target)) continue;
    if (!opts.dryRun) {
      memoryAdapter.connect(feature, source, target, 'related');
      memoryAdapter.connect(feature, target, source, 'related');
    }
    result.connections.push({ source, target, relation: 'related' });
  }

  result.stats.afterConsolidation = memories.length - removed.size;

  return result;
}

function compressMemory(content: string): string {
  const lines = content.split('\n');
  const compressed: string[] = [];
  let inFirstParagraph = true;

  for (const line of lines) {
    if (line.startsWith('#')) {
      compressed.push(line);
      inFirstParagraph = true;
      continue;
    }
    if (inFirstParagraph) {
      if (line.trim() === '') {
        inFirstParagraph = false;
        compressed.push(line);
      } else {
        compressed.push(line);
      }
    }
  }

  compressed.push('\n[compressed -- original archived]');
  return compressed.join('\n');
}
