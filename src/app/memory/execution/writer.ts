/**
 * Build a compact execution memory from task completion data.
 * Pure function -- deterministic, no I/O, no LLM calls.
 */

import type { VerificationReport } from '../../../domain/ports/verification.ts';
import type { MemoryPort } from '../../../domain/ports/memory.ts';
import type { DoctrinePort } from '../../../domain/ports/doctrine.ts';
import type { TaskInfo } from '../../../domain/types.ts';
import { extractKeywords } from '../../dcp/relevance.ts';
import { formatDurationMinutes } from '../../../infra/utils/time-utils.ts';
import { prependMetadataFrontmatter } from '../../../infra/utils/frontmatter.ts';
import { getChangedFilesSince } from '../../../infra/utils/git.ts';
import { readDoctrineTrace, collectDoctrineNames } from '../../doctrine/trace.ts';

export const EXEC_MEMORY_PREFIX = 'exec-';

export function isExecutionMemory(name: string): boolean {
  return name.startsWith(EXEC_MEMORY_PREFIX);
}

export interface ExecutionMemoryParams {
  /** Task ID (slug). Falls back to taskFolder for backward compat. */
  taskId?: string;
  /** @deprecated Use taskId. Kept for backward compat. */
  taskFolder: string;
  taskName: string;
  summary: string;
  verificationReport: VerificationReport | null;
  claimedAt?: string;
  completedAt?: string;
  revisionCount?: number;
  dependsOn?: string[];
  changedFiles?: string[];
  specContent?: string;
}

export interface ExecutionMemoryResult {
  fileName: string;
  content: string;
  tags: string[];
}

const EXTENSION_TAGS: Record<string, string> = {
  '.ts': 'typescript',
  '.tsx': 'typescript',
  '.js': 'javascript',
  '.jsx': 'javascript',
  '.py': 'python',
  '.rs': 'rust',
  '.go': 'golang',
  '.java': 'java',
  '.css': 'styling',
  '.scss': 'styling',
  '.html': 'markup',
  '.sql': 'database',
  '.json': 'config',
  '.yaml': 'config',
  '.yml': 'config',
  '.toml': 'config',
};

function deriveExtensionTags(files: string[]): string[] {
  const tags = new Set<string>();
  for (const file of files) {
    // Check for test files first
    if (/\.(test|spec)\.[^.]+$/.test(file)) {
      tags.add('testing');
      continue;
    }
    const ext = file.match(/\.[^./]+$/)?.[0]?.toLowerCase();
    if (ext && EXTENSION_TAGS[ext]) {
      tags.add(EXTENSION_TAGS[ext]);
    }
  }
  return Array.from(tags);
}

export function deriveFolderTags(folder: string): string[] {
  return folder
    .split('-')
    .filter(seg => seg.length >= 4 && !/^\d+$/.test(seg))
    .slice(0, 3);
}

function formatDuration(claimedAt?: string, completedAt?: string): string {
  if (!claimedAt || !completedAt) return 'unknown';
  const ms = new Date(completedAt).getTime() - new Date(claimedAt).getTime();
  if (isNaN(ms) || ms < 0) return 'unknown';
  const minutes = Math.round(ms / 60000);
  return formatDurationMinutes(minutes);
}

/**
 * Build execution memory content from task completion data.
 * Tags are capped at 8 with strict priority:
 *   1. "execution" (always, 1 slot)
 *   2. Folder segments >= 4 chars (up to 3 slots)
 *   3. File extension language tags (up to 2 slots)
 *   4. Spec keywords (fill remaining, up to 2)
 */
export function buildExecutionMemory(params: ExecutionMemoryParams): ExecutionMemoryResult {
  const {
    taskId, taskFolder, taskName, summary, verificationReport,
    claimedAt, completedAt, revisionCount,
    changedFiles = [], specContent,
  } = params;
  const effectiveId = taskId ?? taskFolder;

  // --- Tags (strict priority, capped at 8, deduplicated) ---
  const tags: string[] = ['execution'];
  const seen = new Set<string>(tags);

  const folderTags = deriveFolderTags(effectiveId);
  for (const t of folderTags) {
    if (tags.length >= 4 || seen.has(t)) continue; // 1 + up to 3
    tags.push(t);
    seen.add(t);
  }

  const extTags = deriveExtensionTags(changedFiles);
  for (const t of extTags) {
    if (tags.length >= 6 || seen.has(t)) continue; // up to 2 more
    tags.push(t);
    seen.add(t);
  }

  if (specContent) {
    const specKw = extractKeywords(specContent);
    for (const kw of specKw) {
      if (tags.length >= 8 || seen.has(kw)) continue; // fill remaining up to 2
      tags.push(kw);
      seen.add(kw);
    }
  }

  // --- Body ---
  const parts: string[] = [
    `Task **${effectiveId}** completed.`,
    '',
    `**Summary**: ${summary}`,
  ];

  if (changedFiles.length > 0) {
    const display = changedFiles.slice(0, 15);
    const suffix = changedFiles.length > 15 ? ` (+${changedFiles.length - 15} more)` : '';
    parts.push('', `**Files changed** (${changedFiles.length}): ${display.join(', ')}${suffix}`);
  }

  if (verificationReport) {
    if (verificationReport.passed) {
      parts.push('', `**Verification**: passed (score ${verificationReport.score.toFixed(2)})`);
    } else {
      const failed = verificationReport.criteria
        .filter(c => !c.passed)
        .map(c => c.name)
        .join(', ');
      parts.push('', `**Verification**: score ${verificationReport.score.toFixed(2)}, failed: ${failed}`);
    }
  }

  const duration = formatDuration(claimedAt, completedAt);
  parts.push('', `**Revisions**: ${revisionCount ?? 0} | **Duration**: ${duration}`);

  const body = parts.join('\n');
  const content = prependMetadataFrontmatter(body, {
    tags,
    category: 'execution',
    priority: 1,
  });

  return {
    fileName: `${EXEC_MEMORY_PREFIX}${effectiveId}`,
    content,
    tags,
  };
}

export interface WriteExecutionMemoryParams {
  memoryAdapter: MemoryPort | undefined;
  doctrinePort?: DoctrinePort;
  featureName: string;
  taskFolder: string;
  task: TaskInfo;
  summary: string;
  projectRoot: string;
  verificationReport: VerificationReport | null;
  specContent?: string;
  featureCreatedAt?: string;
}

/** Write execution memory before task transitions to done. Best-effort, never throws. */
export async function writeExecutionMemory(params: WriteExecutionMemoryParams): Promise<void> {
  const { memoryAdapter, doctrinePort, featureName, taskFolder, task, summary,
    projectRoot, verificationReport, specContent, featureCreatedAt } = params;
  if (!memoryAdapter) return;
  try {
    const sinceISO = task.claimedAt ?? featureCreatedAt;
    const changedFiles = await getChangedFilesSince(projectRoot, sinceISO);
    const result = buildExecutionMemory({
      taskId: task.id,
      taskFolder,
      taskName: task.name ?? taskFolder,
      summary,
      verificationReport,
      claimedAt: task.claimedAt,
      completedAt: new Date().toISOString(),
      revisionCount: task.revisionCount,
      dependsOn: task.dependsOn,
      changedFiles,
      specContent,
    });
    memoryAdapter.write(featureName, result.fileName, result.content);
  } catch {
    // Best-effort -- never block task completion
  }

  // Record doctrine effectiveness from trace file (Phase 4)
  if (doctrinePort) {
    try {
      const trace = readDoctrineTrace(projectRoot, featureName, taskFolder);
      if (trace && trace.entries.length > 0) {
        const doctrineNames = collectDoctrineNames(trace);
        const taskSucceeded = (task.revisionCount ?? 0) === 0 && (verificationReport?.passed ?? true);
        for (const name of doctrineNames) {
          doctrinePort.recordInjection(name, taskSucceeded);
        }
      }
    } catch {
      // Best-effort -- never block task completion
    }
  }
}
