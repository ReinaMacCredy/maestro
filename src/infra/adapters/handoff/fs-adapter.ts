/**
 * FsHandoffAdapter -- filesystem-only HandoffPort implementation.
 * Built-in fallback when Agent Mail is not available.
 * Writes/reads handoff documents under .maestro/features/<feature>/handoffs/.
 */

import type { HandoffPort, HandoffDocument, HandoffResult } from '../../../domain/ports/handoff.ts';
import type { TaskPort } from '../../../domain/ports/task.ts';
import type { MemoryPort } from '../../../domain/ports/memory.ts';
import { getHandoffPath, getHandoffsPath } from '../../utils/paths.ts';
import { ensureDir, writeText, readText, fileExists } from '../../utils/fs-io.ts';
import { getModifiedFiles, extractTitle, formatHandoffMessage } from './shared.ts';
import * as fs from 'node:fs';
import * as path from 'node:path';

export class FsHandoffAdapter implements HandoffPort {
  constructor(
    private readonly projectRoot: string,
    private readonly taskPort: TaskPort,
    private readonly memoryAdapter: MemoryPort,
  ) {}

  async buildHandoff(feature: string, taskId: string): Promise<HandoffDocument> {
    const task = await this.taskPort.get(feature, taskId);

    const memories = this.memoryAdapter.list(feature);
    const decisions = memories.map(mf => ({
      key: mf.name,
      value: mf.content.slice(0, 500),
    }));

    const modifiedFiles = getModifiedFiles(this.projectRoot);

    return {
      beadId: taskId,
      beadState: {
        title: task?.planTitle ?? task?.name ?? taskId,
        status: task?.status ?? 'unknown',
      },
      decisions,
      modifiedFiles,
      blockers: task?.status === 'blocked' ? [task.summary ?? 'Unknown blocker'] : [],
      openQuestions: [],
      nextSteps: [],
      criticalContext: '',
      cassPointer: `Search prior sessions: maestro search-sessions --query "${task?.name ?? taskId}"`,
    };
  }

  async sendHandoff(
    feature: string,
    handoff: HandoffDocument,
    _targetAgent?: string,
  ): Promise<HandoffResult> {
    const body = formatHandoffMessage(handoff, feature);
    const filePath = getHandoffPath(this.projectRoot, feature, handoff.beadId);
    ensureDir(path.dirname(filePath));
    writeText(filePath, body);
    return { filePath, agentMailSent: false };
  }

  async receiveHandoffs(feature: string | undefined, _agentId?: string): Promise<HandoffDocument[]> {
    if (!feature) return [];

    const handoffsDir = getHandoffsPath(this.projectRoot, feature);
    const handoffs: HandoffDocument[] = [];

    try {
      const files = fs.readdirSync(handoffsDir).filter(f => f.endsWith('.md'));
      for (const file of files) {
        const content = readText(path.join(handoffsDir, file));
        if (!content) continue;
        const beadId = file.replace(/\.md$/, '');
        handoffs.push({
          beadId,
          beadState: { title: extractTitle(content), status: 'unknown' },
          decisions: [],
          modifiedFiles: [],
          blockers: [],
          openQuestions: [],
          nextSteps: [],
          criticalContext: content,
        });
      }
    } catch {
      // No handoffs directory yet -- return empty
    }

    return handoffs;
  }

  async acknowledgeHandoff(threadId: string): Promise<void> {
    // Write a .ack sidecar file alongside the handoff document.
    // threadId for fs-only handoffs is the beadId (file stem).
    // We walk all features to find the matching handoff file.
    const featuresBase = path.join(this.projectRoot, '.maestro', 'features');
    try {
      const features = fs.readdirSync(featuresBase, { withFileTypes: true })
        .filter(e => e.isDirectory())
        .map(e => e.name);

      for (const feat of features) {
        const handoffsDir = getHandoffsPath(this.projectRoot, feat);
        const safeName = threadId.replace(/[^a-z0-9-]/gi, '-');
        const handoffFile = path.join(handoffsDir, `${safeName}.md`);
        if (fileExists(handoffFile)) {
          writeText(`${handoffFile}.ack`, new Date().toISOString());
          return;
        }
      }
    } catch {
      // Best-effort
    }
  }

}
