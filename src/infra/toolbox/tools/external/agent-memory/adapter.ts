/**
 * AgentMemoryAdapter -- MemoryPort implementation backed by agentMemory library.
 *
 * Wraps agentMemory's SQLite + vec0 store, hybrid retrieval engine, and
 * workflow signals into maestro's MemoryPort interface. When this adapter
 * is active (priority 200 > fs-memory at 0), maestro uses agentMemory
 * for all memory operations instead of the filesystem backend.
 *
 * The agentMemory library is imported directly (not via MCP/CLI) so
 * there is zero IPC overhead -- it runs in-process.
 */

import type { MemoryPort } from '../../../../../domain/ports/memory.ts';
import type {
  MemoryFile,
  MemoryFileWithMeta,
  MemoryMetadata,
  MemoryConnection,
  MemoryRelation,
} from '../../../../../domain/types.ts';
import type { AdapterContext, AdapterFactory } from '../../../types.ts';
import {
  createStore,
  writeMemory,
  deleteMemory,
  getMemory,
  searchByKeyword,
  type Store,
  type Memory,
} from 'agent-memory';

function memoryToFile(m: Memory): MemoryFile {
  return {
    name: m.name,
    content: m.content,
    updatedAt: m.updatedAt,
    sizeBytes: Buffer.byteLength(m.content, 'utf8'),
  };
}

function memoryToFileWithMeta(m: Memory): MemoryFileWithMeta {
  const metadata: MemoryMetadata = {
    tags: m.tags,
    priority: m.priority,
    category: m.category as MemoryMetadata['category'],
    selectionCount: m.selectionCount,
    lastSelectedAt: m.lastSelectedAt,
  };
  return {
    ...memoryToFile(m),
    metadata,
    bodyContent: m.content,
  };
}

export class AgentMemoryAdapter implements MemoryPort {
  private store: Store;

  constructor() {
    this.store = createStore();
  }

  write(featureName: string, fileName: string, content: string): string {
    const m = writeMemory(this.store, {
      name: fileName,
      content,
      feature: featureName,
    });
    return m.id;
  }

  read(featureName: string, fileName: string): string | null {
    const rows = this.store.db.query<{ content: string }, [string, string]>(
      'SELECT content FROM memories WHERE name = ? AND feature = ? LIMIT 1',
    ).get(fileName, featureName);
    return rows?.content ?? null;
  }

  list(featureName: string): MemoryFile[] {
    const rows = this.store.db.query<Record<string, unknown>, [string]>(
      'SELECT * FROM memories WHERE feature = ? ORDER BY updated_at DESC',
    ).all(featureName);
    return rows.map(r => memoryToFile(this._rowToMemory(r)));
  }

  listWithMeta(featureName: string): MemoryFileWithMeta[] {
    const rows = this.store.db.query<Record<string, unknown>, [string]>(
      'SELECT * FROM memories WHERE feature = ? ORDER BY updated_at DESC',
    ).all(featureName);
    return rows.map(r => memoryToFileWithMeta(this._rowToMemory(r)));
  }

  delete(featureName: string, fileName: string): boolean {
    const result = this.store.db.run(
      'DELETE FROM memories WHERE name = ? AND feature = ?',
      [fileName, featureName],
    );
    return result.changes > 0;
  }

  compile(featureName: string): string {
    const files = this.list(featureName);
    return files.map(f => `## ${f.name}\n\n${f.content}`).join('\n\n---\n\n');
  }

  archive(featureName: string): { archived: string[]; archivePath: string } {
    const files = this.list(featureName);
    const names = files.map(f => f.name);
    // Mark as archived by setting stage to null
    this.store.db.run(
      'UPDATE memories SET category = \'execution\' WHERE feature = ?',
      [featureName],
    );
    return { archived: names, archivePath: `agent-memory:${featureName}` };
  }

  stats(featureName: string): { count: number; totalBytes: number; oldest?: string; newest?: string } {
    const row = this.store.db.query<{ cnt: number; total: number; oldest: string | null; newest: string | null }, [string]>(
      `SELECT COUNT(*) as cnt, COALESCE(SUM(LENGTH(content)), 0) as total,
              MIN(name) as oldest, MAX(name) as newest
       FROM memories WHERE feature = ?`,
    ).get(featureName);
    return {
      count: row?.cnt ?? 0,
      totalBytes: row?.total ?? 0,
      oldest: row?.oldest ?? undefined,
      newest: row?.newest ?? undefined,
    };
  }

  compress(featureName: string, fileName: string): boolean {
    const content = this.read(featureName, fileName);
    if (!content) return false;
    const truncated = content.slice(0, 200) + (content.length > 200 ? '\n\n[compressed]' : '');
    this.store.db.run(
      'UPDATE memories SET content = ?, updated_at = datetime(\'now\') WHERE name = ? AND feature = ?',
      [truncated, fileName, featureName],
    );
    return true;
  }

  isCompressed(featureName: string, fileName: string): boolean {
    const content = this.read(featureName, fileName);
    return content?.includes('[compressed]') ?? false;
  }

  readFull(featureName: string, fileName: string): MemoryFileWithMeta | null {
    const row = this.store.db.query<Record<string, unknown>, [string, string]>(
      'SELECT * FROM memories WHERE name = ? AND feature = ? LIMIT 1',
    ).get(fileName, featureName);
    if (!row) return null;
    return memoryToFileWithMeta(this._rowToMemory(row));
  }

  recordSelection(featureName: string, fileName: string): void {
    this.store.db.run(
      `UPDATE memories SET selection_count = selection_count + 1,
              last_selected_at = datetime('now'), updated_at = datetime('now')
       WHERE name = ? AND feature = ?`,
      [fileName, featureName],
    );
  }

  connect(featureName: string, sourceName: string, targetName: string, relation: MemoryRelation): void {
    const src = this._findId(featureName, sourceName);
    const tgt = this._findId(featureName, targetName);
    if (!src || !tgt) return;
    const id = crypto.randomUUID();
    this.store.db.run(
      'INSERT OR IGNORE INTO connections (id, source_id, target_id, relation) VALUES (?, ?, ?, ?)',
      [id, src, tgt, relation],
    );
  }

  getConnections(featureName: string, name: string): MemoryConnection[] {
    const memId = this._findId(featureName, name);
    if (!memId) return [];
    const rows = this.store.db.query<{ target_id: string; relation: string }, [string]>(
      'SELECT target_id, relation FROM connections WHERE source_id = ?',
    ).all(memId);
    return rows.map(r => {
      const target = this.store.db.query<{ name: string }, [string]>(
        'SELECT name FROM memories WHERE id = ?',
      ).get(r.target_id);
      return { target: target?.name ?? r.target_id, relation: r.relation as MemoryRelation };
    });
  }

  writeGlobal(fileName: string, content: string): string {
    return this.write('__global__', fileName, content);
  }

  readGlobal(fileName: string): string | null {
    return this.read('__global__', fileName);
  }

  listGlobal(): MemoryFile[] {
    return this.list('__global__');
  }

  deleteGlobal(fileName: string): boolean {
    return this.delete('__global__', fileName);
  }

  private _findId(featureName: string, name: string): string | null {
    const row = this.store.db.query<{ id: string }, [string, string]>(
      'SELECT id FROM memories WHERE name = ? AND feature = ? LIMIT 1',
    ).get(name, featureName);
    return row?.id ?? null;
  }

  private _rowToMemory(row: Record<string, unknown>): Memory {
    return {
      id: row.id as string,
      name: row.name as string,
      content: row.content as string,
      category: row.category as string | undefined,
      stage: row.stage as string | undefined,
      taskId: row.task_id as string | undefined,
      feature: row.feature as string | undefined,
      project: row.project as string | undefined,
      tags: JSON.parse((row.tags as string) ?? '[]'),
      priority: row.priority as number,
      selectionCount: row.selection_count as number,
      lastSelectedAt: row.last_selected_at as string | undefined,
      createdAt: row.created_at as string,
      updatedAt: row.updated_at as string,
    };
  }
}

export const createAdapter: AdapterFactory<MemoryPort> = (_ctx: AdapterContext) => {
  return new AgentMemoryAdapter();
};
