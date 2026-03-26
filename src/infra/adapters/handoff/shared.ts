/**
 * Shared utilities for handoff adapters.
 * Canonical implementations of functions previously duplicated across
 * FsHandoffAdapter and AgentMailHandoffAdapter.
 */

import type { HandoffDocument } from '../../../domain/ports/handoff.ts';
import { execFileSync } from 'node:child_process';

/** Get list of modified files from git diff in the given project root. */
export function getModifiedFiles(projectRoot: string): string[] {
  try {
    const stdout = execFileSync('git', ['diff', '--name-only'], {
      cwd: projectRoot,
      encoding: 'utf-8',
      timeout: 5000,
    });
    return stdout.trim().split('\n').filter(Boolean);
  } catch {
    return [];
  }
}

/** Extract the title from a handoff markdown document. */
export function extractTitle(content: string): string {
  const match = content.match(/^##\s+Handoff:\s+(.+)$/m);
  return match ? match[1].trim() : 'Unknown';
}

/**
 * Format a HandoffDocument as markdown.
 * This is the canonical (richest) version, supporting description, design,
 * acceptance criteria, next steps, and br/fs backend-aware context links.
 */
export function formatHandoffMessage(
  handoff: HandoffDocument,
  feature: string,
  taskBackend: string = 'fs',
): string {
  const timestamp = new Date().toISOString().replace('T', ' ').slice(0, 19);
  const sections: string[] = [];
  sections.push(`## Handoff: ${timestamp}`, '');
  sections.push('### Current Task State');
  sections.push(`Task: \`${handoff.beadId}\` | Status: ${handoff.beadState.status}`);
  sections.push(`Title: ${handoff.beadState.title}`, '');
  if (handoff.beadState.description) { sections.push('### Description', handoff.beadState.description.slice(0, 2000), ''); }
  if (handoff.beadState.design) { sections.push('### Design Notes', handoff.beadState.design, ''); }
  if (handoff.beadState.acceptanceCriteria) { sections.push('### Acceptance Criteria', handoff.beadState.acceptanceCriteria, ''); }
  if (handoff.decisions.length > 0) { sections.push('### Key Decisions'); for (const d of handoff.decisions) sections.push(`- **${d.key}**: ${d.value}`); sections.push(''); }
  if (handoff.modifiedFiles.length > 0) { sections.push('### Modified Files'); for (const f of handoff.modifiedFiles) sections.push(`- \`${f}\``); sections.push(''); }
  if (handoff.blockers.length > 0) { sections.push('### Blockers / Open Questions'); for (const b of handoff.blockers) sections.push(`- ${b}`); sections.push(''); }
  if (handoff.criticalContext) { sections.push('### Critical Context', handoff.criticalContext, ''); }
  if (handoff.nextSteps.length > 0) { sections.push('### Next Steps'); for (let i = 0; i < handoff.nextSteps.length; i++) sections.push(`${i + 1}. ${handoff.nextSteps[i]}`); sections.push(''); }
  sections.push('### Handoff Context (for next session)');
  sections.push(`1. Read this handoff file for full context on task \`${handoff.beadId}\`.`);
  if (taskBackend === 'br') sections.push(`2. Run: \`br show ${handoff.beadId} --json\` for current bead state.`);
  else sections.push(`2. Run: \`maestro task-info --feature ${feature} --task ${handoff.beadId}\` for current task state.`);
  if (handoff.cassPointer) sections.push(`3. ${handoff.cassPointer}`);
  else sections.push(`3. Run: \`maestro search-related --task ${handoff.beadId}\` for related context.`);
  sections.push('');
  return sections.join('\n');
}
