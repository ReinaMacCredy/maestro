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
  // Find first runnable task: pending with no unmet dependencies
  const pendingIds = new Set(tasks.filter((t) => t.status !== 'done').map((t) => t.id));
  const firstRunnable = tasks.find((t) => {
    if (t.status !== 'pending') return false;
    if (!t.deps || t.deps.length === 0) return true;
    return t.deps.every((d) => !pendingIds.has(d));
  });
  const taskExample = firstRunnable ? firstRunnable.id : (tasks.find((t) => t.status === 'pending')?.id ?? '<task-id>');

  const lines: string[] = [];
  lines.push('This project uses `maestro` for agent coordination. Always pass `--json` to all commands.');
  lines.push('');
  lines.push('### 1. Find the next runnable task');
  lines.push('```');
  lines.push(`maestro task-next --feature ${feature} --json`);
  lines.push('```');
  lines.push('This returns the next task whose dependencies are satisfied.');
  lines.push('');
  lines.push('### 2. Claim and implement');
  lines.push('```');
  lines.push(`maestro task-claim --feature ${feature} --task ${taskExample} --agent-id <your-id> --json`);
  lines.push('```');
  lines.push('');
  lines.push('### 3. Mark done');
  lines.push('```');
  lines.push(`maestro task-done --feature ${feature} --task ${taskExample} --content "summary of work" --json`);
  lines.push('```');
  lines.push('');
  lines.push('### 4. Repeat until all tasks done');
  lines.push('```');
  lines.push(`maestro task-next --feature ${feature} --json`);
  lines.push('```');
  lines.push('When task-next returns no runnable tasks, all work is done.');
  lines.push('');
  lines.push('### 5. Report completion');
  lines.push('```');
  lines.push(`maestro handoff-report --feature ${feature} --content "Summary of all work done" --json`);
  lines.push('```');
  lines.push('');
  lines.push('### Tips');
  lines.push(`- Run \`maestro status --feature ${feature} --json\` anytime to orient`);
  lines.push(`- If a task is blocked: \`maestro task-block --feature ${feature} --task <id> --reason "..." --json\``);
  lines.push('- Always use task-next to find runnable tasks -- it respects dependency order');

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
