/**
 * Bead builder utilities for plan-to-beads conversion.
 *
 * Builds rich bead content from plan sections following the flywheel principle:
 * beads must be self-contained enough that an agent never needs to look back
 * at the plan. Every bead carries the why, what, failure modes, and
 * verification plan.
 */

import type { ParsedTask } from '../plans/parser.ts';
import { extractPlanSection, getTaskType } from './spec-builder.ts';
import type { CreateOpts } from '../../tasks/port.ts';

const DESIGN_SECTION_RE = /(?:####?\s*(?:Design|Architecture|Approach|Technical Design)[^\n]*)\n([\s\S]*?)(?=####?\s|$)/i;
const AC_SECTION_RE = /(?:####?\s*(?:Acceptance Criteria|Done When|Success Criteria|Verification|AC)[^\n]*)\n([\s\S]*?)(?=####?\s|$)/i;
const CHECKLIST_RE = /^\s*-\s*\[[ x]\].+$/gm;

export interface BeadBuildParams {
  featureName: string;
  task: ParsedTask;
  planContent: string;
  allTasks: ParsedTask[];
  dependsOn: string[];
  memoryFiles?: Array<{ name: string; content: string }>;
}

/**
 * Build rich CreateOpts for a bead from a plan section.
 * Populates description, design, acceptanceCriteria, notes, type.
 */
export function buildBeadOpts(params: BeadBuildParams): CreateOpts {
  const { featureName, task, planContent, allTasks, dependsOn, memoryFiles = [] } = params;

  const planSection = extractPlanSection(planContent, task) ?? '';

  return {
    description: buildBeadDescription(task, planSection, featureName, dependsOn, allTasks, memoryFiles),
    design: extractDesignNotes(planSection),
    acceptanceCriteria: extractAcceptanceCriteria(planSection),
    notes: buildBeadNotes(task, featureName),
    type: inferBeadType(planSection, task.name),
    deps: dependsOn,
    labels: [`feature:${featureName}`],
  };
}

/**
 * Build a rich, self-contained description for the bead.
 * Includes full plan section, dependencies, and memory context.
 */
function buildBeadDescription(
  task: ParsedTask,
  planSection: string,
  featureName: string,
  dependsOn: string[],
  allTasks: ParsedTask[],
  memoryFiles: Array<{ name: string; content: string }>,
): string {
  const sections: string[] = [];

  // Header with context
  sections.push(`# ${task.name}`);
  sections.push(`Feature: ${featureName} | Task ${task.order} of ${allTasks.length}`);
  sections.push('');

  // Plan section (the core spec)
  if (planSection) {
    sections.push('## Specification');
    sections.push('');
    // Strip the heading (already in title) and include the body
    const body = planSection.replace(/^###\s*\d+\.\s*[^\n]+\n?/, '').trim();
    if (body) {
      sections.push(body);
    }
    sections.push('');
  }

  // Dependencies with context
  if (dependsOn.length > 0) {
    sections.push('## Dependencies');
    sections.push('');
    for (const dep of dependsOn) {
      const depTask = allTasks.find(t => t.folder === dep);
      if (depTask) {
        sections.push(`- **${depTask.order}. ${depTask.name}** (\`${dep}\`)`);
      } else {
        sections.push(`- \`${dep}\``);
      }
    }
    sections.push('');
  }

  // Memory context
  if (memoryFiles.length > 0) {
    sections.push('## Context (from research/discovery)');
    sections.push('');
    for (const mem of memoryFiles) {
      sections.push(`### ${mem.name}`);
      sections.push('');
      sections.push(mem.content.trim());
      sections.push('');
    }
  }

  return sections.join('\n').trim();
}

/**
 * Extract design notes from the plan section.
 * Looks for ## Design, ## Architecture, ## Approach subsections or
 * paragraphs mentioning design decisions.
 */
export function extractDesignNotes(planSection: string): string | undefined {
  if (!planSection) return undefined;

  const designMatch = planSection.match(DESIGN_SECTION_RE);
  if (designMatch) {
    const content = designMatch[1].trim();
    if (content) return content;
  }

  return undefined;
}

/**
 * Extract acceptance criteria from the plan section.
 * Looks for ## Acceptance Criteria, ## AC, ## Done When subsections,
 * or checklist-style items.
 */
export function extractAcceptanceCriteria(planSection: string): string | undefined {
  if (!planSection) return undefined;

  const acMatch = planSection.match(AC_SECTION_RE);
  if (acMatch) {
    const content = acMatch[1].trim();
    if (content) return content;
  }

  // Look for checklist items (- [ ] patterns)
  const checklistItems = planSection.match(CHECKLIST_RE);
  if (checklistItems && checklistItems.length > 0) {
    return checklistItems.join('\n');
  }

  return undefined;
}

/**
 * Infer bead type from plan section content.
 */
function inferBeadType(planSection: string, taskName: string): string {
  const taskType = getTaskType(planSection || null, taskName);

  if (taskType === 'testing') return 'task';
  if (taskType === 'greenfield') return 'feature';

  const lower = taskName.toLowerCase();
  if (lower.includes('fix') || lower.includes('bug')) return 'bug';
  if (lower.includes('test')) return 'task';
  if (lower.includes('refactor') || lower.includes('clean')) return 'task';
  if (lower.includes('doc')) return 'task';

  return 'task';
}

/**
 * Build notes field with provenance info.
 */
function buildBeadNotes(task: ParsedTask, featureName: string): string {
  return `Generated from plan.md task ${task.order} for feature "${featureName}"`;
}
