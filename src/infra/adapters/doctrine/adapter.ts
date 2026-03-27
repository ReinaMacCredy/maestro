/**
 * Filesystem adapter for doctrine storage.
 * JSON files at .maestro/doctrine/<name>.json.
 * Atomic writes, lock-protected recordInjection, graceful parse failures.
 */

import * as fs from 'fs';
import * as path from 'path';
import type { DoctrineItem, DoctrinePort, DoctrineStatus } from '../../../domain/ports/doctrine.ts';
import { getDoctrinePath, getDoctrineItemPath } from '../../utils/paths.ts';
import { readJson, writeJsonAtomic } from '../../utils/fs-io.ts';
import { acquireLockSync } from '../../utils/locking.ts';
import { extractKeywords, TAG_WEIGHT, KEYWORD_WEIGHT } from '../../../domain/text-utils.ts';

export const CURRENT_SCHEMA_VERSION = 1;
const RELEVANCE_THRESHOLD = 0.2;

export class FsDoctrineAdapter implements DoctrinePort {
  private readonly projectRoot: string;
  /** In-process cache for active items. Invalidated on write/deprecate. */
  private activeCache: DoctrineItem[] | undefined;
  /** Pre-computed keyword sets for cached active items. */
  private keywordCache: Map<string, Set<string>> | undefined;

  constructor(projectRoot: string) {
    this.projectRoot = projectRoot;
  }

  write(item: DoctrineItem): string {
    const filePath = getDoctrineItemPath(this.projectRoot, item.name);
    const toWrite = { ...item, schemaVersion: CURRENT_SCHEMA_VERSION, updatedAt: new Date().toISOString() };
    writeJsonAtomic(filePath, toWrite);
    this.activeCache = undefined;
    this.keywordCache = undefined;
    return filePath;
  }

  read(name: string): DoctrineItem | null {
    const filePath = getDoctrineItemPath(this.projectRoot, name);
    try {
      return readJson<DoctrineItem>(filePath);
    } catch {
      return null;
    }
  }

  list(opts?: { status?: DoctrineStatus }): DoctrineItem[] {
    const dirPath = getDoctrinePath(this.projectRoot);
    let entries: string[];
    try {
      entries = fs.readdirSync(dirPath).filter(f => f.endsWith('.json'));
    } catch {
      return [];
    }

    const items: DoctrineItem[] = [];
    for (const entry of entries) {
      const name = entry.replace(/\.json$/, '');
      const item = this.read(name);
      if (!item) continue;
      if (opts?.status && item.status !== opts.status) continue;
      items.push(item);
    }

    return items;
  }

  deprecate(name: string): DoctrineItem {
    const item = this.read(name);
    if (!item) throw new Error(`Doctrine item '${name}' not found`);
    item.status = 'deprecated';
    item.updatedAt = new Date().toISOString();
    this.write(item);
    return item;
  }

  findRelevant(tags: string[], keywords: Set<string>): DoctrineItem[] {
    // Use cache if available; otherwise populate from list() and pre-compute keywords
    if (!this.activeCache) {
      this.activeCache = this.list({ status: 'active' });
      this.keywordCache = new Map();
      for (const item of this.activeCache) {
        this.keywordCache.set(item.name, extractKeywords(`${item.rule} ${item.rationale}`));
      }
    }
    const activeItems = this.activeCache;
    if (activeItems.length === 0) return [];

    const scored: Array<{ item: DoctrineItem; score: number }> = [];
    const taskTagSet = new Set(tags.map(t => t.toLowerCase()));

    for (const item of activeItems) {
      const score = this.scoreRelevance(item, taskTagSet, keywords);
      if (score >= RELEVANCE_THRESHOLD) {
        scored.push({ item, score });
      }
    }

    scored.sort((a, b) => b.score - a.score);
    return scored.map(s => s.item);
  }

  recordInjection(name: string, taskSucceeded: boolean): void {
    const filePath = getDoctrineItemPath(this.projectRoot, name);
    let release: (() => void) | undefined;

    try {
      release = acquireLockSync(filePath, { timeout: 500 });

      const item = this.read(name);
      if (!item) return;

      const oldCount = item.effectiveness.injectionCount;
      const oldRate = item.effectiveness.associatedSuccessRate;
      const newCount = oldCount + 1;
      const newRate = (oldRate * oldCount + (taskSucceeded ? 1 : 0)) / newCount;

      item.effectiveness.injectionCount = newCount;
      item.effectiveness.associatedSuccessRate = newRate;
      if (!taskSucceeded) {
        item.effectiveness.overrideCount = (item.effectiveness.overrideCount ?? 0) + 1;
      }
      item.effectiveness.lastInjectedAt = new Date().toISOString();

      this.write(item);
    } catch {
      // Best-effort -- never block task completion
    } finally {
      release?.();
    }
  }

  private scoreRelevance(item: DoctrineItem, taskTagSet: Set<string>, taskKeywords: Set<string>): number {
    const conditionTags = item.conditions.tags ?? [];
    const itemTags = [...new Set([...item.tags, ...conditionTags])];

    let tagMatches = 0;
    for (const tag of itemTags) {
      if (taskTagSet.has(tag.toLowerCase())) tagMatches++;
    }
    const tagScore = itemTags.length > 0 ? tagMatches / itemTags.length : 0;

    const docKeywords = this.keywordCache?.get(item.name) ?? extractKeywords(`${item.rule} ${item.rationale}`);
    let keywordMatches = 0;
    for (const kw of docKeywords) {
      if (taskKeywords.has(kw)) keywordMatches++;
    }
    const keywordScore = docKeywords.size > 0 ? keywordMatches / docKeywords.size : 0;

    return tagScore * TAG_WEIGHT + keywordScore * KEYWORD_WEIGHT;
  }
}
