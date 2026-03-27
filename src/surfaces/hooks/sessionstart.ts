import { readStdin, writeOutput, resolveProjectDir, logHookError, HOOK_EVENTS } from './_helpers.ts';
import { initServices } from '../../services.ts';
import { checkStatus } from '../../app/workflow/status.ts';
import { detectResearchTools } from '../../app/workflow/research-tools.ts';
import { derivePipelineStage, type PipelineStage } from '../../app/workflow/stages.ts';
import { buildPlaybook, buildPlaybookWithExternalSkills } from '../../app/workflow/playbook.ts';
import { detectHost } from '../../infra/utils/host-detect.ts';

const HOOK_NAME = 'sessionstart';

function buildResearchGuidance(tools: string[]): string[] {
  const lines: string[] = [];
  lines.push('Research: Use Agent subagents for codebase exploration, WebSearch + WebFetch for web research.');
  if (tools.includes('context7')) {
    lines.push('  [+] context7 detected -- use for up-to-date library docs and API references.');
  }
  if (tools.includes('notebooklm')) {
    lines.push('  [+] notebooklm detected -- use for deep multi-source research and analysis.');
  }
  if (tools.length === 0) {
    lines.push('  Tip: install context7 and notebooklm-mcp for enhanced research.');
  }
  return lines;
}

function buildPipelineGuidance(stage: PipelineStage): string {
  const pb = buildPlaybook(stage);
  return `Pipeline: [${stage}] ${pb.objective}. Next: ${pb.nextMilestone}`;
}

async function main(): Promise<void> {
  const projectDir = resolveProjectDir();

  const input = await readStdin();
  const source = (input.source as string) || 'startup';

  if (!projectDir) {
    writeOutput({});
    return;
  }

  const services = initServices(projectDir);
  const activeFeature = services.featureAdapter.getActive();

  if (!activeFeature) {
    const ctx = [
      '[maestro] No active feature.',
      'Run `maestro status --json` or `maestro feature-create --json` to start a new feature.',
    ].join('\n');
    writeOutput({
      hookSpecificOutput: {
        hookEventName: HOOK_EVENTS.SessionStart,
        additionalContext: ctx,
      },
    });
    return;
  }

  const featureName = activeFeature.name;
  const status = await checkStatus(services, featureName);
  const stage = derivePipelineStage({
    planExists: status.plan.exists,
    planApproved: status.plan.approved,
    taskTotal: status.tasks.total,
    taskDone: status.tasks.done,
    contextCount: status.context.count,
    featureStatus: status.feature.status,
  });
  const researchTools = detectResearchTools(projectDir);

  const isCompact = source === 'compact' || source === 'resume';

  if (isCompact) {
    const pb = buildPlaybookWithExternalSkills(stage, projectDir);
    const lines = [
      `[maestro] Feature: ${featureName} [${stage}]`,
      `Tasks: ${status.tasks.pending} pending, ${status.tasks.inProgress} claimed, ${status.tasks.done} done (${status.tasks.total} total)`,
      `Next: ${status.nextAction}`,
      ...(pb.skills.length > 0 ? [`Skills: ${pb.skills.join(', ')}`] : []),
    ];
    writeOutput({
      hookSpecificOutput: {
        hookEventName: HOOK_EVENTS.SessionStart,
        additionalContext: lines.join('\n'),
      },
    });
    return;
  }

  // Full context for startup
  const hostType = detectHost();
  const lines: string[] = [
    `[maestro] Feature: ${featureName} [${stage}]` + (hostType !== 'standalone' ? ` (host: ${hostType})` : ''),
    '',
    buildPipelineGuidance(stage),
    '',
    `Plan: ${status.plan.exists ? (status.plan.approved ? 'approved' : 'draft') : 'none'}`,
    `Tasks: ${status.tasks.pending} pending, ${status.tasks.inProgress} claimed, ${status.tasks.done} done (${status.tasks.total} total)`,
  ];

  if (stage === 'discovery' || stage === 'research') {
    lines.push('');
    lines.push(...buildResearchGuidance(researchTools));
  }

  if (status.runnable.length > 0) {
    lines.push('');
    lines.push('Runnable tasks:');
    for (const task of status.runnable) {
      lines.push(`  - ${task}`);
    }
  }

  if (status.blocked.length > 0) {
    lines.push('');
    lines.push('Blocked tasks:');
    for (const b of status.blocked) {
      lines.push(`  - ${b}`);
    }
  }

  lines.push('');
  lines.push(`Next: ${status.nextAction}`);

  const pb = buildPlaybookWithExternalSkills(stage, projectDir);
  if (pb.skills.length > 0) {
    lines.push('');
    lines.push(`Recommended skills: ${pb.skills.join(', ')}`);
  }

  writeOutput({
    hookSpecificOutput: {
      hookEventName: HOOK_EVENTS.SessionStart,
      additionalContext: lines.join('\n'),
    },
  });
}

try {
  await main();
} catch (error) {
  logHookError(resolveProjectDir(), HOOK_NAME, error);
  writeOutput({});
}
