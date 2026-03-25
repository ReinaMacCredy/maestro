/**
 * PreToolUse:Agent hook -- inject task spec into agent prompts.
 *
 * Delegates to taskBrief() for all data gathering and DCP scoring,
 * then formats the structured result into a flat injection string.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { readStdin, writeOutput, resolveProjectDir, logHookError, getSessionsDir } from './_helpers.ts';
import { ensureDir } from '../../infra/utils/fs-io.ts';
import { initServices } from '../../services.ts';
import { taskBrief, type TaskBriefResult } from '../../app/tasks/task-brief.ts';
import { WORKER_RULES } from '../../app/tasks/worker-rules.ts';
import { loadSessionState, recordInjection, saveSessionState } from '../../app/dcp/session.ts';
import { estimateTokens } from '../../infra/utils/tokens.ts';

export { WORKER_RULES };

const TASK_PATTERN = /(?:task[:\s_-]+|(?:^|\s))((?:\d{2}-|maestro-[a-z0-9]+-)?[a-z][a-z0-9-]+)/i;

function formatBriefAsInjection(result: TaskBriefResult, agentGuidance: string | null): string {
  const parts: string[] = [];

  // Spec (always present)
  parts.push(`## Task Spec\n\n${result.spec}`);

  // Worker rules
  parts.push(`\n\n## Worker Rules\n\n${result.workerRules}`);

  // Revision context
  if (result.revisionContext) {
    const rc = result.revisionContext;
    const lines = [`\n\n## Revision Context (attempt ${rc.attempt})`, ''];
    if (rc.feedback) lines.push(`**Feedback**: ${rc.feedback}`);
    if (rc.failedChecks.length > 0) {
      lines.push('', '**Failed checks**:');
      for (const c of rc.failedChecks) lines.push(`- ${c.name}: ${c.detail}`);
    }
    if (rc.suggestions.length > 0) {
      lines.push('', '**Suggestions**:');
      for (const s of rc.suggestions) lines.push(`- ${s}`);
    }
    parts.push(lines.join('\n'));
  }

  // Rich fields
  if (result.richFields?.design) {
    parts.push(`\n\n## Design Notes\n\n${result.richFields.design}`);
  }
  if (result.richFields?.acceptanceCriteria) {
    parts.push(`\n\n## Acceptance Criteria\n\n${result.richFields.acceptanceCriteria}`);
  }

  // Graph context
  if (result.graphContext) {
    const flags: string[] = [];
    if (result.graphContext.onCriticalPath) flags.push('on critical path');
    if (result.graphContext.isBottleneck) flags.push('bottleneck (blocks other tasks)');
    if (flags.length > 0) {
      parts.push(`\n\n## Graph Context\n\n[!] This task is ${flags.join(' and ')}. Prioritize correctness.`);
    }
  }

  // Memories
  if (result.memories.length > 0) {
    const memParts = result.memories.map(m =>
      `### ${m.name} (score: ${m.score})\n\n${m.content}`
    );
    parts.push(`\n\n## Relevant Memories\n\n${memParts.join('\n\n---\n\n')}`);
  }

  // Completed tasks
  if (result.completedTasks.length > 0) {
    const taskLines = result.completedTasks.map(t => `- **${t.name}**: ${t.summary}`);
    parts.push(`\n\n## Completed Tasks\n\n${taskLines.join('\n')}`);
  }

  // Doctrine
  if (result.doctrine.length > 0) {
    const docLines = result.doctrine.map(d => `- **${d.name}**: ${d.rule}`);
    parts.push(`\n\n## Operating Doctrine\n\n${docLines.join('\n')}`);
  }

  // Agent tools guidance
  if (agentGuidance) {
    const guidanceBudget = 500 * 4;
    const trimmed = agentGuidance.length > guidanceBudget
      ? agentGuidance.slice(0, guidanceBudget) + '\n[truncated]'
      : agentGuidance;
    parts.push(`\n\n## Code Intelligence Tools\n\n${trimmed}`);
  }

  return parts.join('');
}

function logDcpMetrics(
  projectDir: string, featureName: string, taskId: string,
  result: TaskBriefResult,
): void {
  try {
    const logDir = getSessionsDir(projectDir);
    ensureDir(logDir);
    const logPath = path.join(logDir, 'dcp-metrics.jsonl');
    const entry = JSON.stringify({
      timestamp: new Date().toISOString(),
      feature: featureName,
      task: taskId,
      totalTokens: result.dcp.totalTokens,
      memoriesIncluded: result.dcp.memoriesIncluded,
      memoriesDropped: result.dcp.memoriesDropped,
      doctrineInjected: result.doctrine.length > 0,
      doctrineNames: result.doctrine.map(d => d.name),
    }) + '\n';
    fs.appendFileSync(logPath, entry);
  } catch { /* best-effort */ }
}

function trackSessionDcp(projectDir: string, result: TaskBriefResult): void {
  try {
    const sessionsDir = getSessionsDir(projectDir);
    const state = loadSessionState(sessionsDir);
    recordInjection(state, {
      totalTokens: result.dcp.totalTokens,
      memoriesIncluded: result.dcp.memoriesIncluded,
      memoriesDropped: result.dcp.memoriesDropped,
      components: {},
    } as Parameters<typeof recordInjection>[1]);
    saveSessionState(sessionsDir, state);
  } catch { /* best-effort */ }
}

async function main(): Promise<void> {
  const input = await readStdin();
  const projectDir = resolveProjectDir();

  if (!projectDir) { writeOutput({}); return; }

  const toolInput = (input.tool_input ?? input.input ?? {}) as Record<string, unknown>;
  const prompt = (toolInput.prompt ?? '') as string;
  if (!prompt) { writeOutput({}); return; }

  const match = prompt.match(TASK_PATTERN);
  if (!match) { writeOutput({}); return; }

  const taskId = match[1];

  try {
    const services = initServices(projectDir);
    const activeFeature = services.featureAdapter.getActive();
    if (!activeFeature) { writeOutput({}); return; }

    const featureName = activeFeature.name;

    // Check task is claimed before doing expensive brief
    const task = await services.taskPort.get(featureName, taskId);
    if (!task || task.status !== 'claimed') { writeOutput({}); return; }

    // Delegate all data gathering to taskBrief
    const result = await taskBrief({
      taskPort: services.taskPort,
      featureAdapter: services.featureAdapter,
      memoryAdapter: services.memoryAdapter,
      settingsPort: services.settingsPort,
      directory: projectDir,
      graphPort: services.graphPort,
      doctrinePort: services.doctrinePort,
    }, featureName, taskId);

    const agentGuidance = services.agentToolsRegistry.assembleProtocol('code-intelligence');
    const injection = formatBriefAsInjection(result, agentGuidance);

    logDcpMetrics(projectDir, featureName, taskId, result);
    trackSessionDcp(projectDir, result);

    writeOutput({
      hookSpecificOutput: {
        hookEventName: 'PreToolUse',
        additionalContext: injection,
      },
    });
  } catch {
    writeOutput({});
  }
}

const isBunDirect = typeof Bun !== 'undefined' && Bun.main === Bun.resolveSync(import.meta.path, '.');
if (isBunDirect) {
  try {
    await main();
  } catch (error) {
    logHookError(resolveProjectDir(), 'pre-agent', error);
    writeOutput({});
  }
}
