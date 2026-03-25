/**
 * verifyTask -- orchestrate verification checks and state transitions.
 *
 * Flow:
 *   1. Read task status for claimedAt and revisionCount
 *   2. If verification disabled: taskPort.done(), return auto-pass
 *   3. If autoAcceptTypes match: skip verification, taskPort.done()
 *   4. Run verification checks
 *   5. Write verification.json
 *   6. If passed: taskPort.done(), return { report, newStatus: 'done' }
 *   7. If failed: taskPort.review(), return { report, newStatus: 'review' }
 *      (MCP handler decides the next step -- autoReject, maxRevisions, etc.)
 */

import type { TaskPort } from '../../tasks/port.ts';
import type { VerificationPort, VerificationReport, VerificationCriterion } from '../../tasks/verification/port.ts';
import type { MemoryPort } from '../../memory/port.ts';
import type { ResolvedVerificationConfig } from '../../tasks/verification/config.ts';
import type { TaskInfo } from '../../core/types.ts';
import { prependMetadataFrontmatter } from '../../core/frontmatter.ts';
import { writeExecutionMemory } from '../memory/execution/writer.ts';

export interface VerifyTaskOpts {
  taskPort: TaskPort;
  verificationPort: VerificationPort;
  memoryAdapter?: MemoryPort;
  config: ResolvedVerificationConfig;
  projectRoot: string;
  featureName: string;
  taskFolder: string;
  summary: string;
}

export interface VerifyTaskResult {
  report: VerificationReport;
  newStatus: 'done' | 'review';
  task: TaskInfo;
}

function makeAutoPass(criteria: VerificationCriterion[] = []): VerificationReport {
  return {
    passed: true,
    score: 1,
    criteria,
    suggestions: [],
    timestamp: new Date().toISOString(),
  };
}

export async function verifyTask(opts: VerifyTaskOpts): Promise<VerifyTaskResult> {
  const { taskPort, verificationPort, memoryAdapter, config,
    projectRoot, featureName, taskFolder, summary } = opts;

  // Read current task state
  const task = await taskPort.get(featureName, taskFolder);
  if (!task) throw new Error(`Task '${taskFolder}' not found`);

  if (!config.enabled) {
    await writeExecutionMemory({ memoryAdapter, featureName, taskFolder, task, summary, projectRoot, verificationReport: null });
    const doneTask = await taskPort.done(featureName, taskFolder, summary);
    return { report: makeAutoPass(), newStatus: 'done', task: doneTask };
  }

  // Hoist reads -- used by both auto-accept check and verification
  const [spec, richFields] = await Promise.all([
    taskPort.readSpec(featureName, taskFolder),
    taskPort.getRichFields?.(featureName, taskFolder) ?? Promise.resolve(null),
  ]);

  // Auto-accept types -- skip verification for matching task types
  if (config.autoAcceptTypes.length > 0) {
    const taskType = richFields?.type ?? inferTaskType(spec);
    if (taskType && config.autoAcceptTypes.includes(taskType)) {
      await writeExecutionMemory({ memoryAdapter, featureName, taskFolder, task, summary, projectRoot, verificationReport: null, specContent: spec ?? undefined });
      const doneTask = await taskPort.done(featureName, taskFolder, summary);
      return {
        report: makeAutoPass([{ name: 'auto-accept', passed: true, detail: `Task type '${taskType}' auto-accepted` }]),
        newStatus: 'done', task: doneTask,
      };
    }
  }

  // Run verification checks
  const report = await verificationPort.verify({
    projectRoot,
    featureName,
    taskFolder,
    summary,
    specContent: spec ?? undefined,
    acceptanceCriteria: richFields?.acceptanceCriteria ?? undefined,
    claimedAt: task.claimedAt,
  });

  // Write verification report
  await taskPort.writeVerification(featureName, taskFolder, report);

  if (report.passed) {
    await writeExecutionMemory({ memoryAdapter, featureName, taskFolder, task, summary, projectRoot, verificationReport: report, specContent: spec ?? undefined });
    const doneTask = await taskPort.done(featureName, taskFolder, summary);
    return { report, newStatus: 'done', task: doneTask };
  }

  // Verification failed -- transition to review (MCP handler decides next step)
  const reviewTask = await taskPort.review(featureName, taskFolder, summary);

  // Write failure memory for future reference
  if (memoryAdapter) {
    try {
      const failedCriteria = report.criteria.filter(c => !c.passed).map(c => c.name).join(', ');
      const body = [
        `Verification failed for ${taskFolder}: ${failedCriteria}.`,
        `Score: ${report.score.toFixed(2)}.`,
        `Suggestions: ${report.suggestions.join('; ')}`,
      ].join('\n');
      const content = prependMetadataFrontmatter(body, {
        tags: ['verification', 'failure-pattern'],
        category: 'debug',
        priority: 1,
      });
      memoryAdapter.write(featureName, `verification-fail-${taskFolder}`, content);
    } catch {
      // Memory write is best-effort
    }
  }

  return { report, newStatus: 'review', task: reviewTask };
}

function inferTaskType(spec: string | null): string | undefined {
  if (!spec) return undefined;
  const lower = spec.toLowerCase();
  if (lower.includes('## task type')) {
    const match = spec.match(/## Task Type\s*\n\s*(\w+)/i);
    return match?.[1]?.toLowerCase();
  }
  return undefined;
}
