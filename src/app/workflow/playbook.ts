/**
 * Playbook engine -- stage-specific workflow guidance for agents.
 *
 * buildPlaybook() returns the tools, skills, and objectives for a pipeline stage.
 * buildTransitionHint() returns breadcrumb hints for stage-transition tools.
 *
 * IMPORTANT: This file MUST NOT import from registry.generated.ts (380KB+).
 * The session hook imports this at startup -- pulling in the registry would bloat it.
 * Skill names are hardcoded strings; validation is test-only.
 */

import type { PipelineStage } from './stages.ts';
import type { WorkflowRegistry } from './registry.ts';
import type { ToolboxRegistry } from '../../infra/toolbox/registry.ts';
import { discoverExternalSkillsByStage } from '../skills/external-discovery.ts';

export interface Playbook {
  stage: PipelineStage;
  objective: string;
  tools: string[];
  skills: string[];
  nextMilestone: string;
  antiPatterns: string[];
}

export interface TransitionHint {
  nextStep: string;
  loadSkill?: string;
}

/** Editorial content per stage -- objectives, skills, anti-patterns. */
const STAGE_CONTENT: Record<PipelineStage, Omit<Playbook, 'tools'>> = {
  discovery: {
    stage: 'discovery',
    objective: 'Explore scope, brainstorm, capture findings',
    skills: ['maestro:brainstorming', 'maestro:design', 'maestro:parallel-exploration'],
    nextMilestone: 'Write plan with maestro plan-write',
    antiPatterns: ["Don't write the plan yet -- explore first", "Don't skip maestro memory-write -- findings are lost without it"],
  },
  research: {
    stage: 'research',
    objective: 'Deep-dive codebase, save structured findings',
    skills: ['maestro:brainstorming', 'maestro:design', 'maestro:parallel-exploration'],
    nextMilestone: 'Write plan with maestro plan-write',
    antiPatterns: ["Don't research without saving findings as memory", "Don't start planning before research is sufficient"],
  },
  planning: {
    stage: 'planning',
    objective: 'Write plan with discovery section, non-goals, ghost diffs',
    skills: ['maestro:design'],
    nextMilestone: 'Approve plan with maestro plan-approve',
    antiPatterns: ["Don't skip ## Discovery section in plan", "Don't approve your own plan without review"],
  },
  approval: {
    stage: 'approval',
    objective: 'Generate tasks from the approved plan',
    skills: ['maestro:implement'],
    nextMilestone: 'Run maestro task-sync to generate tasks',
    antiPatterns: ["Don't skip maestro task-sync -- jumping to implementation without tasks loses tracking"],
  },
  execution: {
    stage: 'execution',
    objective: 'Claim tasks, implement via TDD, verify',
    skills: ['maestro:implement', 'maestro:dispatching', 'maestro:tdd'],
    nextMilestone: 'All tasks done',
    antiPatterns: ["Don't skip maestro task-claim before working", "Don't mark done without verification"],
  },
  handoff: {
    stage: 'handoff',
    objective: 'Cross-agent handoff in progress -- monitor or review results',
    skills: [],
    nextMilestone: 'Run maestro handoff-pickup --json to check status, or review with maestro handoff-report',
    antiPatterns: ["Don't start new work on a handed-off feature", "Don't forget to review the handoff report before completing"],
  },
  done: {
    stage: 'done',
    objective: 'Complete feature, promote memories, review doctrine',
    skills: [],
    nextMilestone: 'Run maestro feature-complete',
    antiPatterns: ["Don't forget to promote useful memories to global"],
  },
};

/**
 * Build playbook using the dynamic workflow registry.
 * Tools come from the registry; editorial content from STAGE_CONTENT.
 */
export function buildPlaybook(
  stage: PipelineStage,
  registry?: WorkflowRegistry,
  toolbox?: ToolboxRegistry,
): Playbook {
  const content = STAGE_CONTENT[stage];
  const tools = registry
    ? registry.getToolsForStage(stage, toolbox)
    : [];
  return { ...content, tools };
}

/**
 * Build playbook with external skills merged in.
 */
export function buildPlaybookWithExternalSkills(
  stage: PipelineStage,
  projectRoot: string,
  registry?: WorkflowRegistry,
  toolbox?: ToolboxRegistry,
): Playbook {
  const base = buildPlaybook(stage, registry, toolbox);
  const external = discoverExternalSkillsByStage(projectRoot, stage);
  if (external.length === 0) return base;
  const extraNames = external.map(s => s.name).filter(n => !base.skills.includes(n));
  if (extraNames.length === 0) return base;
  return { ...base, skills: [...base.skills, ...extraNames] };
}

const allTasksCompleteHint = (ctx?: TransitionContext): TransitionHint | undefined => {
  if (ctx?.taskDone === undefined || ctx?.taskTotal === undefined) return undefined;
  if (ctx.taskDone < ctx.taskTotal) return undefined;
  return { nextStep: 'All tasks complete. Run maestro feature-complete' };
};

const TRANSITION_HINTS: Record<string, (ctx?: TransitionContext) => TransitionHint | undefined> = {
  plan_approve: () => ({
    nextStep: 'Run maestro task-sync to generate tasks, then maestro task-next',
    loadSkill: 'maestro:implement',
  }),
  tasks_sync: (ctx) => {
    if (!ctx?.created || ctx.created <= 0) return undefined;
    return { nextStep: 'Run maestro task-next to find runnable work, then maestro task-claim' };
  },
  task_done: allTasksCompleteHint,
  task_accept: allTasksCompleteHint,
  feature_complete: () => ({
    nextStep: 'Feature done. Review doctrine suggestions if any',
  }),
};

interface TransitionContext {
  taskDone?: number;
  taskTotal?: number;
  created?: number;
}

export function buildTransitionHint(
  tool: string,
  context?: TransitionContext,
): TransitionHint | undefined {
  const factory = TRANSITION_HINTS[tool];
  if (!factory) return undefined;
  return factory(context);
}
