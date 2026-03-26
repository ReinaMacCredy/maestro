/**
 * AgentMemory adapter -- retrieval engine for maestro memory files.
 *
 * Unlike other adapters that provide a domain port, agentMemory is a
 * read-only retrieval engine. It indexes .maestro/ memory files and
 * provides hybrid search (keyword + semantic + workflow signals).
 *
 * Maestro uses this for enhanced DCP memory selection when available.
 * Falls back to the standard FsMemoryAdapter scoring when not available.
 */

import type { AdapterContext, AdapterFactory } from '../../../types.ts';
import {
  createStore,
  syncIndex,
  rebuildIndex,
  type Store,
} from 'agent-memory';
import { keywordSearch } from 'agent-memory';
import { mergeSignals, type RetrievalSignal } from 'agent-memory';
import { scoreStageProximity } from 'agent-memory';
import { scoreMemoryEffectiveness } from 'agent-memory';
import { readMemoryFile } from 'agent-memory';
import { selectWithMmr } from 'agent-memory';

export interface AgentMemoryRetriever {
  /** Hybrid search across indexed memories. */
  query(queryText: string, opts?: {
    stage?: string;
    feature?: string;
    limit?: number;
  }): Promise<Array<{ path: string; score: number; snippet: string }>>;

  /** Budget-aware context assembly for agent briefs. */
  compile(taskId: string, opts?: {
    stage?: string;
    feature?: string;
    budgetTokens?: number;
  }): Promise<{ compiled: string; memoriesUsed: number; tokensUsed: number }>;

  /** Force reindex. */
  reindex(): Promise<number>;

  /** Sync index (incremental). */
  sync(): Promise<number>;
}

class AgentMemoryAdapter implements AgentMemoryRetriever {
  private store: Store;

  constructor(maestroDir: string) {
    this.store = createStore(maestroDir);
  }

  async query(queryText: string, opts?: {
    stage?: string;
    feature?: string;
    limit?: number;
  }): Promise<Array<{ path: string; score: number; snippet: string }>> {
    await syncIndex(this.store);

    const limit = opts?.limit ?? 20;
    const allSignals: RetrievalSignal[][] = [];

    // Keyword search
    const kwSignals = keywordSearch(queryText, this.store.index);
    allSignals.push(kwSignals);

    const candidateIds = new Set(kwSignals.map(s => s.memoryId));

    // Stage scoring
    if (opts?.stage) {
      const stageSignals: RetrievalSignal[] = [];
      for (const relPath of candidateIds) {
        const entry = this.store.index.entries[relPath];
        if (!entry) continue;
        stageSignals.push({
          memoryId: relPath,
          score: scoreStageProximity(entry.metadata.stage, opts.stage),
          source: 'pipelineStage',
        });
      }
      allSignals.push(stageSignals);
    }

    // Feedback
    const fbSignals: RetrievalSignal[] = [];
    for (const relPath of candidateIds) {
      const eff = scoreMemoryEffectiveness(this.store, relPath);
      if (eff !== 0) {
        fbSignals.push({ memoryId: relPath, score: (eff + 1) / 2, source: 'execFeedback' });
      }
    }
    if (fbSignals.length > 0) allSignals.push(fbSignals);

    const merged = mergeSignals(allSignals);
    return merged.slice(0, limit).map(r => {
      const mem = readMemoryFile(this.store.maestroDir, r.memoryId);
      return {
        path: r.memoryId,
        score: r.totalScore,
        snippet: mem ? mem.body.slice(0, 200) : '',
      };
    });
  }

  async compile(taskId: string, opts?: {
    stage?: string;
    feature?: string;
    budgetTokens?: number;
  }): Promise<{ compiled: string; memoriesUsed: number; tokensUsed: number }> {
    await syncIndex(this.store);

    const budget = opts?.budgetTokens ?? 1024;
    const queryText = taskId.replace(/[-_]/g, ' ');
    const allSignals: RetrievalSignal[][] = [];

    const kwSignals = keywordSearch(queryText, this.store.index);
    allSignals.push(kwSignals);

    if (opts?.stage) {
      const stageSignals: RetrievalSignal[] = [];
      for (const [relPath, entry] of Object.entries(this.store.index.entries)) {
        if (opts.feature && !relPath.includes(`features/${opts.feature}/`)) continue;
        stageSignals.push({
          memoryId: relPath,
          score: scoreStageProximity(entry.metadata.stage, opts.stage),
          source: 'pipelineStage',
        });
      }
      allSignals.push(stageSignals);
    }

    const merged = mergeSignals(allSignals);
    const mmrCandidates = merged.map(r => ({
      ...r,
      tokenCount: this.store.index.entries[r.memoryId]?.tokenCount ?? 100,
    }));
    const selected = selectWithMmr(mmrCandidates, budget);

    let tokensUsed = 0;
    const sections: string[] = [];

    for (const sel of selected) {
      const mem = readMemoryFile(this.store.maestroDir, sel.memoryId);
      if (!mem) continue;
      const name = sel.memoryId.split('/').pop()?.replace('.md', '') ?? sel.memoryId;
      sections.push(`## ${name}\n\n${mem.body}`);
      tokensUsed += sel.tokenCount;
    }

    return {
      compiled: sections.join('\n\n---\n\n'),
      memoriesUsed: selected.length,
      tokensUsed,
    };
  }

  async reindex(): Promise<number> {
    return rebuildIndex(this.store);
  }

  async sync(): Promise<number> {
    return syncIndex(this.store);
  }
}

export const createAdapter: AdapterFactory<AgentMemoryRetriever> = (ctx: AdapterContext) => {
  return new AgentMemoryAdapter(ctx.projectRoot + '/.maestro');
};
