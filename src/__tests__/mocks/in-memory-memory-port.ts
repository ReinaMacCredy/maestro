/**
 * InMemoryMemoryPort -- mock MemoryPort for unit testing (including DCP).
 */

import type { MemoryFile, MemoryFileWithMeta, MemoryMetadata } from '../../domain/types.ts';
import type { MemoryPort } from '../../domain/ports/memory.ts';
import { parseFrontmatterRich, stripFrontmatter, serializeFrontmatter } from '../../infra/utils/frontmatter.ts';
import { inferMetadata } from '../../app/memory/execution/inference.ts';

interface StoredMemory {
  name: string;
  content: string;
  updatedAt: string;
  sizeBytes: number;
}

export class InMemoryMemoryPort implements MemoryPort {
  private featureMemories = new Map<string, Map<string, StoredMemory>>();
  private globalMemories = new Map<string, StoredMemory>();

  private getFeatureMap(feature: string): Map<string, StoredMemory> {
    if (!this.featureMemories.has(feature)) {
      this.featureMemories.set(feature, new Map());
    }
    return this.featureMemories.get(feature)!;
  }

  write(featureName: string, fileName: string, content: string): string {
    const name = fileName.replace(/\.md$/, '');
    const mem: StoredMemory = {
      name,
      content,
      updatedAt: new Date().toISOString(),
      sizeBytes: Buffer.byteLength(content),
    };
    this.getFeatureMap(featureName).set(name, mem);
    return `/mock/${featureName}/memory/${name}.md`;
  }

  read(featureName: string, fileName: string): string | null {
    const name = fileName.replace(/\.md$/, '');
    return this.getFeatureMap(featureName).get(name)?.content ?? null;
  }

  list(featureName: string): MemoryFile[] {
    return [...this.getFeatureMap(featureName).values()];
  }

  listWithMeta(featureName: string): MemoryFileWithMeta[] {
    return this.list(featureName).map(f => {
      const bodyContent = stripFrontmatter(f.content);
      const parsed = parseFrontmatterRich(f.content);
      const inferred = inferMetadata(bodyContent, f.name);

      const metadata: MemoryMetadata = {
        tags: parsed && Array.isArray(parsed.tags) ? parsed.tags as string[] : inferred.tags,
        priority: parsed && typeof parsed.priority === 'number' ? parsed.priority : inferred.priority,
        category: parsed && typeof parsed.category === 'string'
          ? parsed.category as MemoryMetadata['category']
          : inferred.category,
      };

      return { ...f, metadata, bodyContent };
    });
  }

  delete(featureName: string, fileName: string): boolean {
    const name = fileName.replace(/\.md$/, '');
    return this.getFeatureMap(featureName).delete(name);
  }

  compile(featureName: string): string {
    const files = this.list(featureName);
    if (files.length === 0) return '';
    return files.map(f => `## ${f.name}\n\n${f.content}`).join('\n\n---\n\n');
  }

  archive(featureName: string): { archived: string[]; archivePath: string } {
    const files = this.list(featureName);
    this.getFeatureMap(featureName).clear();
    return { archived: files.map(f => f.name), archivePath: '/mock/archive' };
  }

  stats(featureName: string): { count: number; totalBytes: number; oldest?: string; newest?: string } {
    const files = this.list(featureName);
    if (files.length === 0) return { count: 0, totalBytes: 0 };
    return {
      count: files.length,
      totalBytes: files.reduce((sum, f) => sum + f.sizeBytes, 0),
      oldest: files[0].name,
      newest: files[files.length - 1].name,
    };
  }

  writeGlobal(fileName: string, content: string): string {
    const name = fileName.replace(/\.md$/, '');
    this.globalMemories.set(name, {
      name,
      content,
      updatedAt: new Date().toISOString(),
      sizeBytes: Buffer.byteLength(content),
    });
    return `/mock/global/memory/${name}.md`;
  }

  readGlobal(fileName: string): string | null {
    const name = fileName.replace(/\.md$/, '');
    return this.globalMemories.get(name)?.content ?? null;
  }

  listGlobal(): MemoryFile[] {
    return [...this.globalMemories.values()];
  }

  deleteGlobal(fileName: string): boolean {
    const name = fileName.replace(/\.md$/, '');
    return this.globalMemories.delete(name);
  }

  compress(featureName: string, fileName: string): boolean {
    const name = fileName.replace(/\.md$/, '');
    const mem = this.getFeatureMap(featureName).get(name);
    if (!mem) return false;

    const parsed = parseFrontmatterRich(mem.content) ?? {};
    const body = stripFrontmatter(mem.content);
    const summary = body.slice(0, 200);
    const newMeta = { ...parsed, compressed: true };
    const updated = serializeFrontmatter(newMeta as Record<string, unknown>) + '\n' + summary;

    this.getFeatureMap(featureName).set(name, {
      ...mem,
      content: updated,
      sizeBytes: Buffer.byteLength(updated),
    });
    return true;
  }

  isCompressed(featureName: string, fileName: string): boolean {
    const name = fileName.replace(/\.md$/, '');
    const mem = this.getFeatureMap(featureName).get(name);
    if (!mem) return false;
    const parsed = parseFrontmatterRich(mem.content);
    return parsed !== null && String(parsed.compressed) === 'true';
  }

  readFull(featureName: string, fileName: string): import('../../domain/types.ts').MemoryFileWithMeta | null {
    const name = fileName.replace(/\.md$/, '');
    return this.listWithMeta(featureName).find(f => f.name === name) ?? null;
  }

  recordSelection(_featureName: string, _fileName: string): void {
    // no-op for mock
  }

  connect(_featureName: string, _sourceName: string, _targetName: string, _relation: import('../../domain/types.ts').MemoryRelation): void {
    // no-op for mock
  }

  getConnections(_featureName: string, _name: string): import('../../domain/types.ts').MemoryConnection[] {
    return [];
  }

  // Test helpers
  reset(): void {
    this.featureMemories.clear();
    this.globalMemories.clear();
  }

  /** Seed a memory with a specific updatedAt for testing recency */
  seed(featureName: string, name: string, content: string, updatedAt?: string): void {
    this.getFeatureMap(featureName).set(name, {
      name,
      content,
      updatedAt: updatedAt ?? new Date().toISOString(),
      sizeBytes: Buffer.byteLength(content),
    });
  }
}
