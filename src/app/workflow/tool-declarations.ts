/**
 * Workflow metadata declarations for all MCP tools.
 * Centralized registry population -- called once during server startup.
 */

import type { WorkflowRegistry } from './registry.ts';

/**
 * Register workflow metadata for all MCP tools.
 */
export function declareAllTools(registry: WorkflowRegistry): void {
  // =========================================================================
  // Feature tools (merged)
  // =========================================================================
  registry.register('maestro_feature', { stages: ['discovery', 'done'], category: 'primary' });
  registry.register('maestro_feature_read', { stages: [], category: 'meta' });

  // =========================================================================
  // Plan tools (merged)
  // =========================================================================
  registry.register('maestro_plan', { stages: ['planning'], category: 'primary' });
  registry.register('maestro_plan_read', { stages: ['planning'], category: 'meta' });

  // =========================================================================
  // Task tools (merged)
  // =========================================================================
  registry.register('maestro_task', { stages: ['approval', 'execution'], category: 'primary', prerequisites: ['maestro_plan'] });
  registry.register('maestro_task_read', { stages: ['execution'], category: 'meta' });

  // =========================================================================
  // Memory tools (merged)
  // =========================================================================
  registry.register('maestro_memory', { stages: ['discovery', 'research', 'done'], category: 'primary' });
  registry.register('maestro_memory_read', { stages: [], category: 'meta' });

  // =========================================================================
  // Doctrine tools (merged)
  // =========================================================================
  registry.register('maestro_doctrine', { stages: ['done'], category: 'primary' });
  registry.register('maestro_doctrine_read', { stages: [], category: 'meta' });

  // =========================================================================
  // Skill tool (merged)
  // =========================================================================
  registry.register('maestro_skill', { stages: [], category: 'meta' });

  // =========================================================================
  // Graph tool (merged, conditional: requires bv)
  // =========================================================================
  registry.register('maestro_graph', { stages: ['execution'], category: 'conditional', requires: 'bv' });

  // =========================================================================
  // Search tool (merged, conditional: requires cass)
  // =========================================================================
  registry.register('maestro_search', { stages: ['discovery', 'research'], category: 'conditional', requires: 'cass' });

  // =========================================================================
  // Handoff tools (merged, conditional: requires agent-mail)
  // =========================================================================
  registry.register('maestro_handoff', { stages: ['execution'], category: 'conditional', requires: 'agent-mail' });
  registry.register('maestro_handoff_read', { stages: ['execution'], category: 'conditional', requires: 'agent-mail' });

  // =========================================================================
  // Visual tool (merged)
  // =========================================================================
  registry.register('maestro_visual', { stages: [], category: 'meta' });

  // =========================================================================
  // DCP tool (merged)
  // =========================================================================
  registry.register('maestro_dcp', { stages: [], category: 'utility' });

  // =========================================================================
  // Config tools (unchanged)
  // =========================================================================
  registry.register('maestro_config_get', { stages: [], category: 'meta' });
  registry.register('maestro_config_set', { stages: [], category: 'utility' });

  // =========================================================================
  // Meta / standalone tools (unchanged)
  // =========================================================================
  registry.register('maestro_status', { stages: ['discovery', 'research', 'planning', 'approval', 'execution', 'done'], category: 'meta' });
  registry.register('maestro_ping', { stages: [], category: 'meta' });
  registry.register('maestro_doctor', { stages: [], category: 'meta' });
  registry.register('maestro_init', { stages: [], category: 'utility' });
  registry.register('maestro_execution_insights', { stages: ['done'], category: 'meta' });
  registry.register('maestro_history', { stages: [], category: 'meta' });
}
