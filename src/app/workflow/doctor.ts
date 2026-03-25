/**
 * doctor use case.
 * Validates config, checks integrations, reports health.
 * v2: uses toolbox status for richer per-tool output.
 */

import type { SettingsPort } from '../../core/settings.ts';
import type { FeaturePort } from '../../features/port.ts';
import type { TaskPort } from '../../tasks/port.ts';
import type { GraphPort } from '../../tasks/graph/port.ts';
import type { HandoffPort } from '../../handoff/port.ts';
import type { SearchPort } from '../../search/port.ts';
import type { DoctrinePort } from '../../doctrine/port.ts';
import type { ToolboxRegistry } from '../../toolbox/registry.ts';
import type { AgentToolsRegistry } from '../../toolbox/agents/registry.ts';

export interface DoctorServices {
  settingsPort: SettingsPort;
  featureAdapter: FeaturePort;
  taskPort: TaskPort;
  directory: string;
  toolbox: ToolboxRegistry;
  agentToolsRegistry: AgentToolsRegistry;
  taskBackend: 'fs' | 'br';
  graphPort?: GraphPort;
  handoffPort?: HandoffPort;
  searchPort?: SearchPort;
  doctrinePort?: DoctrinePort;
}

export type CheckStatus = 'ok' | 'warn' | 'fail';

export interface Check {
  name: string;
  status: CheckStatus;
  message: string;
}

export interface DoctorReport {
  checks: Check[];
  summary: { ok: number; warn: number; fail: number };
}

export async function doctor(services: DoctorServices): Promise<DoctorReport> {
  const checks: Check[] = [];

  // 1. Config check
  try {
    services.settingsPort.get();
    checks.push({ name: 'config', status: 'ok', message: 'Config loaded' });
  } catch {
    checks.push({ name: 'config', status: 'fail', message: 'Config failed to load' });
  }

  // 2. Active feature check + 3. Task backend check
  let active: ReturnType<typeof services.featureAdapter.getActive> = null;
  try {
    active = services.featureAdapter.getActive();
    if (active) {
      checks.push({ name: 'active-feature', status: 'ok', message: `Active: ${active.name}` });
    } else {
      checks.push({ name: 'active-feature', status: 'warn', message: 'No active feature' });
    }
  } catch {
    checks.push({ name: 'active-feature', status: 'fail', message: 'Feature adapter error' });
  }

  try {
    if (active) {
      await services.taskPort.list(active.name);
      checks.push({ name: 'task-backend', status: 'ok', message: `Task backend reachable (${services.taskBackend})` });
    } else {
      checks.push({ name: 'task-backend', status: 'warn', message: 'No active feature to test tasks' });
    }
  } catch {
    checks.push({ name: 'task-backend', status: 'fail', message: 'Task backend unreachable' });
  }

  // 4. Toolbox-driven integration checks
  const toolStatuses = services.toolbox.getStatus();
  for (const ts of toolStatuses) {
    // Skip the task providers (already checked above)
    if (ts.manifest.provides === 'tasks') continue;

    const name = `${ts.manifest.name} (${ts.manifest.provides ?? 'utility'})`;
    if (ts.settingsState === 'denied') {
      checks.push({ name, status: 'warn', message: 'Denied by settings' });
    } else if (!ts.installed) {
      checks.push({ name, status: 'warn', message: `Not installed${ts.manifest.install ? ` -- ${ts.manifest.install}` : ''}` });
    } else {
      checks.push({ name, status: 'ok', message: `Available${ts.version ? ` (${ts.version})` : ''}` });
    }
  }

  // 5. Agent tools
  const agentTools = services.agentToolsRegistry.getAll();
  for (const at of agentTools) {
    const name = `${at.manifest.name} (${at.manifest.category})`;
    if (!at.installed) {
      checks.push({ name, status: 'warn', message: 'Not installed' });
    } else {
      checks.push({ name, status: 'ok', message: `Available${at.version ? ` (${at.version})` : ''}` });
    }
  }

  // Compute summary
  const summary = { ok: 0, warn: 0, fail: 0 };
  for (const check of checks) {
    summary[check.status]++;
  }

  return { checks, summary };
}
