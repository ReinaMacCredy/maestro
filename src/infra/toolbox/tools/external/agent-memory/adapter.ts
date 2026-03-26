/**
 * AgentMemory adapter -- thin wrapper over agentMemory's retrieval engine.
 *
 * Read-only retrieval engine for .maestro/ memory files.
 * Delegates all scoring logic to the agentMemory library.
 */

import type { AdapterContext, AdapterFactory } from '../../../types.ts';
import {
  createStore,
  queryMemories,
  compileMemories,
  rebuildIndex,
  syncIndex,
  type Store,
  type QueryResult,
  type CompileResult,
} from 'agent-memory';

export interface AgentMemoryRetriever {
  query(queryText: string, opts?: {
    stage?: string;
    feature?: string;
    limit?: number;
  }): Promise<QueryResult[]>;

  compile(taskId: string, opts?: {
    stage?: string;
    feature?: string;
    budgetTokens?: number;
  }): Promise<CompileResult>;

  reindex(): Promise<number>;
  sync(): Promise<number>;
}

class AgentMemoryAdapter implements AgentMemoryRetriever {
  private store: Store;

  constructor(maestroDir: string) {
    this.store = createStore(maestroDir);
  }

  query(queryText: string, opts?: {
    stage?: string;
    feature?: string;
    limit?: number;
  }): Promise<QueryResult[]> {
    return queryMemories(this.store, queryText, opts);
  }

  compile(taskId: string, opts?: {
    stage?: string;
    feature?: string;
    budgetTokens?: number;
  }): Promise<CompileResult> {
    return compileMemories(this.store, taskId, opts);
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
