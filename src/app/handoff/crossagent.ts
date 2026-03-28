/**
 * Cross-agent handoff -- build, read, and report on feature-level handoffs
 * between different agent hosts (Claude, Codex, etc.).
 */

import type { PlanPort } from '../../domain/ports/plan.ts';
import type { FeaturePort } from '../../domain/ports/feature.ts';
import type { TaskPort } from '../../domain/ports/task.ts';
import type { MemoryPort } from '../../domain/ports/memory.ts';
import type { DoctrinePort } from '../../domain/ports/doctrine.ts';
import type { FeatureStatusType } from '../../domain/types.ts';
import { MaestroError } from '../../domain/errors.ts';
import { VERSION } from '../../version.ts';
import { detectHost } from '../../infra/utils/host-detect.ts';
import { getCrossAgentHandoffDir } from '../../infra/utils/paths.ts';
import { getModifiedFiles } from '../../infra/adapters/handoff/shared.ts';
import { scoreByGoal } from './scorer.ts';
import { formatCrossAgentHandoff, formatCrossAgentReport, buildQuickstart } from './crossagent-format.ts';
import { readJson, writeJsonAtomic, ensureDir, writeText, readText } from '../../infra/utils/fs-io.ts';
import * as path from 'path';
import * as fs from 'fs';

// ============================================================================
// Types
// ============================================================================

export type CrossAgentStatus = 'pending' | 'picked-up' | 'completed';

export interface CrossAgentState {
  feature: string;
  status: CrossAgentStatus;
  fromHost: string;
  toAgent?: string;
  createdAt: string;
  handoffPath: string;
  pickedUpAt?: string;
  pickedUpBy?: string;
  completedAt?: string;
  reportPath?: string;
}

export interface CrossAgentTask {
  id: string;
  name: string;
  status: string;
  deps?: string[];
}

export interface CrossAgentDocument {
  feature: string;
  fromHost: string;
  toAgent?: string;
  maestroVersion: string;
  createdAt: string;
  plan: string;
  tasks: CrossAgentTask[];
  memories: Array<{ name: string; category?: string; content: string }>;
  doctrine: Array<{ name: string; rule: string }>;
  modifiedFiles: string[];
  additionalContext?: string;
}

export interface CrossAgentServices {
  featureAdapter: FeaturePort;
  planAdapter: PlanPort;
  taskPort?: TaskPort;
  memoryAdapter?: MemoryPort;
  doctrinePort?: DoctrinePort;
  directory: string;
}

// ============================================================================
// Build
// ============================================================================

export interface BuildOpts {
  toAgent?: string;
  additionalContext?: string;
  memoryLimit?: number;
}

export async function buildCrossAgentHandoff(
  services: CrossAgentServices,
  feature: string,
  opts?: BuildOpts,
): Promise<{ handoffPath: string; statePath: string; document: CrossAgentDocument; state: CrossAgentState }> {
  const { featureAdapter, planAdapter, directory } = services;

  // Validate feature exists and is active
  const featureJson = featureAdapter.requireActive(feature);

  // Reject if already handed off (previous handoff still pending)
  if (featureJson.status === 'handed-off') {
    throw new MaestroError('Feature is already handed off to another agent', [
      'Wait for handoff-report, or run: maestro handoff-pickup --json to check status',
    ]);
  }

  // Validate plan is approved
  const plan = planAdapter.read(feature);
  if (!plan || plan.status !== 'approved') {
    throw new MaestroError('Plan must be approved before handoff', [
      'Run: maestro plan-approve --json',
    ]);
  }

  // Validate tasks exist
  let tasks: CrossAgentTask[] = [];
  if (services.taskPort) {
    const allTasks = await services.taskPort.list(feature, { includeAll: true });
    if (allTasks.length === 0) {
      throw new MaestroError('No tasks found. Sync tasks from the plan first.', [
        'Run: maestro task-sync --json',
      ]);
    }
    tasks = allTasks.map((t) => ({
      id: t.id,
      name: t.name,
      status: t.status,
      ...(t.dependsOn && t.dependsOn.length > 0 ? { deps: t.dependsOn } : {}),
    }));
  }

  // Collect memories (DCP-scored if goal available)
  let memories: Array<{ name: string; category?: string; content: string }> = [];
  if (services.memoryAdapter) {
    try {
      const allMemories = services.memoryAdapter.listWithMeta(feature);
      const goal = `Implement feature: ${feature}. ${plan.content.slice(0, 500)}`;
      const scored = scoreByGoal(allMemories, goal, { limit: opts?.memoryLimit ?? 10 });
      memories = scored.map((s) => ({
        name: s.name,
        category: s.memory.metadata.category,
        content: s.memory.bodyContent.slice(0, 500),
      }));
    } catch {
      // Best-effort
    }
  }

  // Collect doctrine
  let doctrine: Array<{ name: string; rule: string }> = [];
  if (services.doctrinePort) {
    try {
      const all = services.doctrinePort.list();
      doctrine = all.slice(0, 5).map((d) => ({
        name: d.name,
        rule: d.rule,
      }));
    } catch {
      // Best-effort
    }
  }

  // Get modified files
  const modifiedFiles = getModifiedFiles(directory);

  const now = new Date().toISOString();
  const fromHost = detectHost();

  const document: CrossAgentDocument = {
    feature,
    fromHost,
    toAgent: opts?.toAgent,
    maestroVersion: VERSION,
    createdAt: now,
    plan: plan.content,
    tasks,
    memories,
    doctrine,
    modifiedFiles,
    additionalContext: opts?.additionalContext,
  };

  const dir = getCrossAgentHandoffDir(directory, feature);
  ensureDir(dir);

  const handoffPath = path.join(dir, 'handoff.md');
  const statePath = path.join(dir, 'state.json');

  const quickstart = buildQuickstart(feature, tasks);
  const markdown = formatCrossAgentHandoff(document, quickstart);
  writeText(handoffPath, markdown);

  const state: CrossAgentState = {
    feature,
    status: 'pending',
    fromHost,
    toAgent: opts?.toAgent,
    createdAt: now,
    handoffPath,
  };
  writeJsonAtomic(statePath, state);

  // Update feature status
  featureAdapter.updateStatus(feature, 'handed-off');

  return { handoffPath, statePath, document, state };
}

// ============================================================================
// Pickup
// ============================================================================

export interface PickupResult {
  feature: string;
  plan: string;
  tasks: CrossAgentTask[];
  quickstart: string;
  state: CrossAgentState;
}

export function pickupCrossAgentHandoff(
  projectRoot: string,
  featureName?: string,
): PickupResult {
  let dir: string;

  if (featureName) {
    dir = getCrossAgentHandoffDir(projectRoot, featureName);
  } else {
    // Scan for any pending handoff
    const crossagentRoot = path.join(projectRoot, '.maestro', 'handoff', 'crossagent');
    if (!fs.existsSync(crossagentRoot)) {
      throw new MaestroError('No cross-agent handoffs found', [
        'Run: maestro handoff-plan --to <agent> --json',
      ]);
    }
    const dirs = fs.readdirSync(crossagentRoot, { withFileTypes: true })
      .filter((d) => d.isDirectory())
      .map((d) => d.name);

    let found: string | null = null;
    for (const name of dirs) {
      const statePath = path.join(crossagentRoot, name, 'state.json');
      const state = readJson<CrossAgentState>(statePath);
      if (state && (state.status === 'pending' || state.status === 'picked-up')) {
        found = name;
        break;
      }
    }
    if (!found) {
      throw new MaestroError('No pending cross-agent handoffs', [
        'All handoffs have been completed or none exist.',
      ]);
    }
    dir = path.join(crossagentRoot, found);
    featureName = found;
  }

  const statePath = path.join(dir, 'state.json');
  const handoffPath = path.join(dir, 'handoff.md');

  const state = readJson<CrossAgentState>(statePath);
  if (!state) {
    throw new MaestroError(`No handoff state found for '${featureName}'`, [
      `Run: maestro handoff-plan --feature ${featureName} --to <agent> --json`,
    ]);
  }

  const handoffContent = readText(handoffPath);
  if (!handoffContent) {
    throw new MaestroError(`Handoff file not found for '${featureName}'`, [
      `Expected: ${handoffPath}`,
    ]);
  }

  // Reject completed handoffs
  if (state.status === 'completed') {
    throw new MaestroError(`Handoff for '${featureName}' is already completed`, [
      'Check the report: maestro status --json',
      'Create a new handoff: maestro handoff-plan --to <agent> --json',
    ]);
  }

  const currentHost = detectHost();

  // Update state to picked-up, or re-claim from wrong agent
  if (state.status === 'pending') {
    state.status = 'picked-up';
    state.pickedUpAt = new Date().toISOString();
    state.pickedUpBy = currentHost;
    writeJsonAtomic(statePath, state);
  } else if (state.status === 'picked-up' && state.pickedUpBy !== currentHost) {
    // Re-claim: intended agent taking over from wrong agent
    state.pickedUpAt = new Date().toISOString();
    state.pickedUpBy = currentHost;
    writeJsonAtomic(statePath, state);
  }
  // else: picked-up by same host -- idempotent, no state change

  // Parse plan and tasks from the markdown (or use state for feature name)
  // Extract plan section between ## Plan and next ##
  const planMatch = handoffContent.match(/## Plan\n([\s\S]*?)(?=\n## [A-Z])/);
  const plan = planMatch ? planMatch[1].trim() : '';

  // Parse task table
  const tasks = parseTaskTable(handoffContent);
  const quickstart = buildQuickstart(featureName!, tasks);

  return { feature: featureName!, plan, tasks, quickstart, state };
}

function parseTaskTable(content: string): CrossAgentTask[] {
  const tasks: CrossAgentTask[] = [];
  const tableMatch = content.match(/## Tasks\n[\s\S]*?\|[\s\S]*?\|[\s\S]*?\n((?:\|.*\n)*)/);
  if (!tableMatch) return tasks;

  const rows = tableMatch[1].trim().split('\n');
  for (const row of rows) {
    // Skip separator rows (|---|---|...)
    if (/^\|[\s-|]+$/.test(row)) continue;
    const cells = row.split('|').map((c) => c.trim()).filter(Boolean);
    if (cells.length >= 4) {
      const id = cells[1];
      const name = cells[2];
      const status = cells[3];
      const deps = cells[4] && cells[4] !== '-' ? cells[4].split(',').map((d) => d.trim()) : undefined;
      tasks.push({ id, name, status, deps });
    }
  }
  return tasks;
}

// ============================================================================
// Report
// ============================================================================

export interface ReportResult {
  feature: string;
  reportPath: string;
  tasksCompleted: number;
  tasksPending: number;
  state: CrossAgentState;
}

export async function reportCrossAgentHandoff(
  services: CrossAgentServices,
  feature: string,
  summary: string,
): Promise<ReportResult> {
  const dir = getCrossAgentHandoffDir(services.directory, feature);
  const statePath = path.join(dir, 'state.json');

  const state = readJson<CrossAgentState>(statePath);
  if (!state) {
    throw new MaestroError(`No handoff state found for '${feature}'`, [
      'This feature was not handed off. Run: maestro handoff-plan --json',
    ]);
  }

  // Count task statuses
  let tasksCompleted = 0;
  let tasksPending = 0;
  if (services.taskPort) {
    const allTasks = await services.taskPort.list(feature, { includeAll: true });
    for (const t of allTasks) {
      if (t.status === 'done') tasksCompleted++;
      else tasksPending++;
    }
  }

  const fromHost = detectHost();
  const reportContent = formatCrossAgentReport(feature, summary, tasksCompleted, tasksPending, fromHost);
  const reportPath = path.join(dir, 'report.md');
  writeText(reportPath, reportContent);

  // Update state
  state.status = 'completed';
  state.completedAt = new Date().toISOString();
  state.reportPath = reportPath;
  writeJsonAtomic(statePath, state);

  // Update feature status
  services.featureAdapter.updateStatus(feature, 'review-pending');

  return { feature, reportPath, tasksCompleted, tasksPending, state };
}
