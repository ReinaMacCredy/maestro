/**
 * Filesystem-based memory adapter for maestroCLI.
 * Supports both per-feature memory (.maestro/features/<name>/memory/)
 * and global memory (.maestro/memory/).
 */

import * as fs from 'fs';
import * as path from 'path';
import { getMemoryPath, getGlobalMemoryPath } from '../../utils/paths.ts';
import { ensureDir, fileExists, readText, writeText } from '../../utils/fs-io.ts';
import type { MemoryFile, MemoryFileWithMeta, MemoryMetadata, MemoryConnection, MemoryRelation } from '../../../domain/types.ts';
import type { MemoryPort } from '../../../domain/ports/memory.ts';
import { parseFrontmatterRich, stripFrontmatter, serializeFrontmatter, prependMetadataFrontmatter } from '../../utils/frontmatter.ts';
import { inferMetadata } from '../../../app/memory/execution/inference.ts';

export class FsMemoryAdapter implements MemoryPort {
  constructor(private projectRoot: string) {}

  write(featureName: string, fileName: string, content: string): string {
    const memoryPath = getMemoryPath(this.projectRoot, featureName);
    return this._write(memoryPath, fileName, content);
  }

  read(featureName: string, fileName: string): string | null {
    const memoryPath = getMemoryPath(this.projectRoot, featureName);
    const filePath = path.join(memoryPath, this.normalizeFileName(fileName));
    return readText(filePath);
  }

  list(featureName: string): MemoryFile[] {
    const memoryPath = getMemoryPath(this.projectRoot, featureName);
    return this._list(memoryPath);
  }

  listWithMeta(featureName: string): MemoryFileWithMeta[] {
    const files = this.list(featureName);
    return files.map(f => this._enrichWithMeta(f));
  }

  delete(featureName: string, fileName: string): boolean {
    const memoryPath = getMemoryPath(this.projectRoot, featureName);
    const filePath = path.join(memoryPath, this.normalizeFileName(fileName));
    try {
      fs.unlinkSync(filePath);
      return true;
    } catch (err: unknown) {
      if ((err as NodeJS.ErrnoException).code === 'ENOENT') return false;
      throw err;
    }
  }

  compile(featureName: string): string {
    const files = this.list(featureName);
    if (files.length === 0) return '';

    const sections = files.map(f => `## ${f.name}\n\n${f.content}`);
    return sections.join('\n\n---\n\n');
  }

  archive(featureName: string): { archived: string[]; archivePath: string } {
    const memories = this.list(featureName);
    if (memories.length === 0) return { archived: [], archivePath: '' };

    const memoryPath = getMemoryPath(this.projectRoot, featureName);
    const archiveDir = path.join(memoryPath, '..', 'archive');
    ensureDir(archiveDir);

    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    const archived: string[] = [];

    for (const mem of memories) {
      const archiveName = `${timestamp}_${mem.name}.md`;
      const src = path.join(memoryPath, `${mem.name}.md`);
      const dest = path.join(archiveDir, archiveName);
      fs.copyFileSync(src, dest);
      fs.unlinkSync(src);
      archived.push(mem.name);
    }

    return { archived, archivePath: archiveDir };
  }

  stats(featureName: string): { count: number; totalBytes: number; oldest?: string; newest?: string } {
    const memoryPath = getMemoryPath(this.projectRoot, featureName);
    return this._stats(memoryPath);
  }

  compress(featureName: string, fileName: string): boolean {
    const content = this.read(featureName, fileName);
    if (!content) return false;

    const parsed = parseFrontmatterRich(content) ?? {};
    const body = stripFrontmatter(content);
    const summary = body.slice(0, 200);
    const newMeta = { ...parsed, compressed: true };
    const updated = serializeFrontmatter(newMeta as Record<string, unknown>) + '\n' + summary;

    const memoryPath = getMemoryPath(this.projectRoot, featureName);
    const filePath = path.join(memoryPath, this.normalizeFileName(fileName));
    writeText(filePath, updated);
    return true;
  }

  isCompressed(featureName: string, fileName: string): boolean {
    const content = this.read(featureName, fileName);
    if (!content) return false;
    const parsed = parseFrontmatterRich(content);
    return parsed !== null && String(parsed.compressed) === 'true';
  }

  readFull(featureName: string, fileName: string): MemoryFileWithMeta | null {
    const name = fileName.replace(/\.md$/, '');
    const memoryPath = getMemoryPath(this.projectRoot, featureName);
    const filePath = path.join(memoryPath, this.normalizeFileName(name));
    const content = readText(filePath);
    if (!content) return null;
    let stat;
    try { stat = fs.statSync(filePath); } catch { return null; }
    const file: MemoryFile = {
      name,
      content,
      updatedAt: stat.mtime.toISOString(),
      sizeBytes: stat.size,
    };
    return this._enrichWithMeta(file);
  }

  deleteGlobal(fileName: string): boolean {
    const globalPath = getGlobalMemoryPath(this.projectRoot);
    const filePath = path.join(globalPath, this.normalizeFileName(fileName));
    try {
      fs.unlinkSync(filePath);
      return true;
    } catch (err: unknown) {
      if ((err as NodeJS.ErrnoException).code === 'ENOENT') return false;
      throw err;
    }
  }

  // -- Global memory (project-scoped, not feature-scoped) --

  writeGlobal(fileName: string, content: string): string {
    const globalPath = getGlobalMemoryPath(this.projectRoot);
    return this._write(globalPath, fileName, content);
  }

  readGlobal(fileName: string): string | null {
    const globalPath = getGlobalMemoryPath(this.projectRoot);
    const filePath = path.join(globalPath, this.normalizeFileName(fileName));
    return readText(filePath);
  }

  listGlobal(): MemoryFile[] {
    const globalPath = getGlobalMemoryPath(this.projectRoot);
    return this._list(globalPath);
  }

  /**
   * Record that a memory was selected by DCP. Increments selectionCount
   * and updates lastSelectedAt in frontmatter.
   */
  recordSelection(featureName: string, fileName: string): void {
    const memoryPath = getMemoryPath(this.projectRoot, featureName);
    const filePath = path.join(memoryPath, this.normalizeFileName(fileName));
    const content = readText(filePath);
    if (!content) return;

    const parsed = parseFrontmatterRich(content);
    const metadata: MemoryMetadata = (parsed ?? {}) as MemoryMetadata;
    const body = stripFrontmatter(content);
    const newMeta: MemoryMetadata = {
      ...metadata,
      selectionCount: (metadata.selectionCount ?? 0) + 1,
      lastSelectedAt: new Date().toISOString(),
    };

    const updated = prependMetadataFrontmatter(body, newMeta);
    writeText(filePath, updated);
  }

  connect(featureName: string, sourceName: string, targetName: string, relation: MemoryRelation): void {
    const memoryPath = getMemoryPath(this.projectRoot, featureName);
    const filePath = path.join(memoryPath, this.normalizeFileName(sourceName));
    const content = readText(filePath);
    if (!content) return;

    const parsed = parseFrontmatterRich(content);
    const body = stripFrontmatter(content);

    // Parse existing connections from frontmatter (stored as flat strings "target:relation")
    const existing = this.parseConnections(parsed?.connections);
    const entry = `${targetName}:${relation}`;
    if (existing.some(c => c.target === targetName && c.relation === relation)) return; // already connected

    const connectionStrings = [...existing.map(c => `${c.target}:${c.relation}`), entry];

    const metadata: MemoryMetadata = (parsed ?? {}) as MemoryMetadata;
    const meta: Record<string, unknown> = {};
    if (metadata.tags?.length) meta.tags = metadata.tags;
    if (metadata.priority !== undefined) meta.priority = metadata.priority;
    if (metadata.category) meta.category = metadata.category;
    if (metadata.selectionCount !== undefined) meta.selectionCount = metadata.selectionCount;
    if (metadata.lastSelectedAt) meta.lastSelectedAt = metadata.lastSelectedAt;
    meta.connections = connectionStrings;

    const updated = serializeFrontmatter(meta) + '\n' + body;
    writeText(filePath, updated);
  }

  getConnections(featureName: string, name: string): MemoryConnection[] {
    const memoryPath = getMemoryPath(this.projectRoot, featureName);
    const filePath = path.join(memoryPath, this.normalizeFileName(name));
    const content = readText(filePath);
    if (!content) return [];

    const parsed = parseFrontmatterRich(content);
    return this.parseConnections(parsed?.connections);
  }

  /** Parse connections from frontmatter value (array of "target:relation" strings). */
  private parseConnections(raw: unknown): MemoryConnection[] {
    if (!raw || !Array.isArray(raw)) return [];
    const VALID_RELATIONS = new Set<MemoryRelation>(['related', 'supersedes', 'contradicts', 'extends']);
    return (raw as string[])
      .map(s => {
        const idx = String(s).lastIndexOf(':');
        if (idx < 1) return null;
        const target = String(s).slice(0, idx);
        const relation = String(s).slice(idx + 1) as MemoryRelation;
        if (!VALID_RELATIONS.has(relation)) return null;
        return { target, relation };
      })
      .filter((c): c is MemoryConnection => c !== null);
  }

  private _write(dir: string, fileName: string, content: string): string {
    ensureDir(dir);
    const filePath = path.join(dir, this.normalizeFileName(fileName));
    writeText(filePath, content);

    const stats = this._stats(dir);
    const warnings: string[] = [];
    if (stats.totalBytes > 20000) {
      warnings.push(`[warn] Memory total: ~${stats.totalBytes} bytes (exceeds 20,000). Consider archiving with memory-archive.`);
    }
    if (stats.count >= 30) {
      warnings.push(`[warn] ${stats.count} memories (threshold: 30). Consider running memory-consolidate to merge duplicates.`);
    }

    return warnings.length > 0
      ? `${filePath}\n\n${warnings.join('\n')}`
      : filePath;
  }

  private _list(dir: string): MemoryFile[] {
    if (!fileExists(dir)) return [];

    const files = fs.readdirSync(dir, { withFileTypes: true })
      .filter(f => f.isFile() && f.name.endsWith('.md'))
      .map(f => f.name);

    return files.map(name => {
      const filePath = path.join(dir, name);
      const stat = fs.statSync(filePath);
      const content = readText(filePath) || '';
      return {
        name: name.replace(/\.md$/, ''),
        content,
        updatedAt: stat.mtime.toISOString(),
        sizeBytes: stat.size,
      };
    });
  }

  private _stats(dir: string): { count: number; totalBytes: number; oldest?: string; newest?: string } {
    if (!fileExists(dir)) return { count: 0, totalBytes: 0 };

    const entries = fs.readdirSync(dir, { withFileTypes: true })
      .filter(f => f.isFile() && f.name.endsWith('.md'))
      .map(f => {
        const stat = fs.statSync(path.join(dir, f.name));
        return { name: f.name.replace(/\.md$/, ''), size: stat.size, mtime: stat.mtime.getTime() };
      });

    if (entries.length === 0) return { count: 0, totalBytes: 0 };

    entries.sort((a, b) => a.mtime - b.mtime);

    return {
      count: entries.length,
      totalBytes: entries.reduce((sum, e) => sum + e.size, 0),
      oldest: entries[0].name,
      newest: entries[entries.length - 1].name,
    };
  }

  private _enrichWithMeta(file: MemoryFile): MemoryFileWithMeta {
    const bodyContent = stripFrontmatter(file.content);
    const parsed = parseFrontmatterRich(file.content);

    const selectionCount = parsed && typeof parsed.selectionCount === 'number' ? parsed.selectionCount : undefined;
    const lastSelectedAt = parsed && typeof parsed.lastSelectedAt === 'string' ? parsed.lastSelectedAt : undefined;

    const connections = this.parseConnections(parsed?.connections);

    if (parsed && Array.isArray(parsed.tags) && typeof parsed.priority === 'number' && typeof parsed.category === 'string') {
      return {
        ...file, bodyContent,
        metadata: {
          tags: parsed.tags as string[], priority: parsed.priority,
          category: parsed.category as MemoryMetadata['category'],
          ...(selectionCount !== undefined ? { selectionCount } : {}),
          ...(lastSelectedAt !== undefined ? { lastSelectedAt } : {}),
          ...(connections.length > 0 ? { connections } : {}),
        },
      };
    }

    const inferred = inferMetadata(bodyContent, file.name);
    const metadata: MemoryMetadata = {
      tags: parsed && Array.isArray(parsed.tags) ? parsed.tags as string[] : inferred.tags,
      priority: parsed && typeof parsed.priority === 'number' ? parsed.priority : inferred.priority,
      category: parsed && typeof parsed.category === 'string'
        ? parsed.category as MemoryMetadata['category']
        : inferred.category,
      ...(selectionCount !== undefined ? { selectionCount } : {}),
      ...(lastSelectedAt !== undefined ? { lastSelectedAt } : {}),
      ...(connections.length > 0 ? { connections } : {}),
    };

    return { ...file, metadata, bodyContent };
  }

  private normalizeFileName(name: string): string {
    const normalized = name.replace(/\.md$/, '');
    return `${normalized}.md`;
  }
}
