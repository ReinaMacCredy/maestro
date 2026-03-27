/**
 * Markdown formatters for cross-agent handoff documents.
 */

import type { CrossAgentDocument, CrossAgentTask } from './crossagent.ts';

/**
 * Format a cross-agent handoff document as self-contained markdown.
 */
export function formatCrossAgentHandoff(doc: CrossAgentDocument, quickstart: string): string {
  const sections: string[] = [];

  // Header
  sections.push(`# Cross-Agent Handoff: ${doc.feature}`);
  sections.push('');
  sections.push('| Field | Value |');
  sections.push('|-------|-------|');
  sections.push(`| From | ${doc.fromHost} |`);
  sections.push(`| To | ${doc.toAgent ?? 'any'} |`);
  sections.push(`| Created | ${doc.createdAt} |`);
  sections.push(`| maestro | ${doc.maestroVersion} |`);
  sections.push('');

  // Plan
  sections.push('## Plan');
  sections.push(doc.plan);
  sections.push('');

  // Tasks
  sections.push('## Tasks');
  sections.push('');
  sections.push('| # | ID | Name | Status | Depends On |');
  sections.push('|---|-----|------|--------|------------|');
  doc.tasks.forEach((t, i) => {
    const deps = t.deps && t.deps.length > 0 ? t.deps.join(', ') : '-';
    sections.push(`| ${i + 1} | ${t.id} | ${t.name} | ${t.status} | ${deps} |`);
  });
  sections.push('');

  // Key Decisions (memories)
  if (doc.memories.length > 0) {
    sections.push('## Key Decisions');
    sections.push('');
    for (const m of doc.memories) {
      const cat = m.category ? ` (${m.category})` : '';
      sections.push(`### ${m.name}${cat}`);
      sections.push(m.content);
      sections.push('');
    }
  }

  // Doctrine
  if (doc.doctrine.length > 0) {
    sections.push('## Doctrine');
    sections.push('');
    for (const d of doc.doctrine) {
      sections.push(`- **${d.name}**: ${d.rule}`);
    }
    sections.push('');
  }

  // Modified files
  if (doc.modifiedFiles.length > 0) {
    sections.push('## Modified Files');
    sections.push('');
    for (const f of doc.modifiedFiles) {
      sections.push(`- \`${f}\``);
    }
    sections.push('');
  }

  // Additional context
  if (doc.additionalContext) {
    sections.push('## Additional Context');
    sections.push('');
    sections.push(doc.additionalContext);
    sections.push('');
  }

  // Quickstart
  sections.push('## Quickstart');
  sections.push('');
  sections.push(quickstart);

  return sections.join('\n');
}

/**
 * Build quickstart instructions for the receiving agent.
 */
export function buildQuickstart(feature: string, tasks: CrossAgentTask[]): string {
  const pendingTasks = tasks.filter((t) => t.status === 'pending');
  const firstTask = pendingTasks[0];
  const taskExample = firstTask ? firstTask.id : '<task-id>';

  const lines: string[] = [];
  lines.push('This project uses `maestro` for agent coordination. Always pass `--json` to all commands.');
  lines.push('');
  lines.push('### 1. Claim a task');
  lines.push('```');
  lines.push(`maestro task-claim --feature ${feature} --task ${taskExample} --agent-id <your-id> --json`);
  lines.push('```');
  lines.push('');
  lines.push('### 2. Implement, then mark done');
  lines.push('```');
  lines.push(`maestro task-done --feature ${feature} --task ${taskExample} --content "summary of work" --json`);
  lines.push('```');
  lines.push('');
  lines.push('### 3. Check remaining work');
  lines.push('```');
  lines.push(`maestro task-list --feature ${feature} --json`);
  lines.push('```');
  lines.push('');
  lines.push('### 4. Report completion');
  lines.push('```');
  lines.push(`maestro handoff-report --feature ${feature} --content "Summary of all work done" --json`);
  lines.push('```');
  lines.push('');
  lines.push('### Tips');
  lines.push('- Run `maestro status --json` anytime to orient');
  lines.push('- If a task is blocked: `maestro task-block --task <id> --reason "..." --json`');
  lines.push('- Tasks have dependencies -- claim only tasks with no pending deps');

  return lines.join('\n');
}

/**
 * Format a cross-agent completion report.
 */
export function formatCrossAgentReport(
  feature: string,
  summary: string,
  tasksCompleted: number,
  tasksPending: number,
  fromHost: string,
): string {
  const sections: string[] = [];
  sections.push(`# Handoff Report: ${feature}`);
  sections.push('');
  sections.push('| Field | Value |');
  sections.push('|-------|-------|');
  sections.push(`| Reporter | ${fromHost} |`);
  sections.push(`| Completed | ${new Date().toISOString()} |`);
  sections.push(`| Tasks Completed | ${tasksCompleted} |`);
  sections.push(`| Tasks Pending | ${tasksPending} |`);
  sections.push('');
  sections.push('## Summary');
  sections.push('');
  sections.push(summary);
  sections.push('');
  return sections.join('\n');
}
