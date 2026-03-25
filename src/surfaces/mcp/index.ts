/**
 * MCP server entry point for maestro.
 * Lazy service initialization -- starts without .maestro/ existing.
 */

import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { createServicesThunk } from './services-thunk.ts';
import { registerStatusTools } from './handlers/status.ts';
import { registerFeatureTools } from './handlers/feature.ts';
import { registerPlanTools } from './handlers/plan.ts';
import { registerTaskTools } from './handlers/task.ts';
import { registerMemoryTools } from './handlers/memory.ts';
import { registerSkillTools } from './handlers/skill.ts';
import { registerInitTools } from './handlers/init.ts';
import { registerGraphTools } from './handlers/graph.ts';
import { registerHandoffTools } from './handlers/handoff.ts';
import { registerSearchTools } from './handlers/search.ts';
import { registerPingTools } from './handlers/ping.ts';
import { registerDcpTools } from './handlers/dcp.ts';
import { registerExecutionInsightsTools } from './handlers/execution-insights.ts';
import { registerDoctrineTools } from './handlers/doctrine.ts';
import { registerBriefTools } from './handlers/brief.ts';
import { registerConfigTools } from './handlers/config.ts';
import { registerVisualTools } from './handlers/visual.ts';
import { registerDoctorTools } from './handlers/doctor.ts';
import { registerHistoryTools } from './handlers/history.ts';
import { VERSION } from '../../version.ts';
import { FsSettingsAdapter } from '../../core/settings-adapter.ts';
import { buildToolbox } from '../../toolbox/registry.ts';
import { WorkflowRegistry } from '../../workflow/registry.ts';
import { declareAllTools } from '../../workflow/tool-declarations.ts';
import { createWorkflowEngine } from '../../workflow/engine.ts';
import { registerWorkflowTools } from './handlers/workflow.ts';

export function createMaestroServer(directory: string): McpServer {
  const server = new McpServer({
    name: 'maestro',
    version: VERSION,
  });

  // Build toolbox eagerly for conditional tool registration
  const settings = new FsSettingsAdapter(directory).get();
  const toolbox = buildToolbox(settings);
  // Build workflow registry with tool metadata declarations
  const workflowRegistry = new WorkflowRegistry();
  declareAllTools(workflowRegistry);
  const { engine } = createWorkflowEngine(workflowRegistry);

  const thunk = createServicesThunk(directory, toolbox, workflowRegistry);

  registerStatusTools(server, thunk, engine);
  registerFeatureTools(server, thunk);
  registerPlanTools(server, thunk);
  registerTaskTools(server, thunk);
  registerMemoryTools(server, thunk);
  registerSkillTools(server, thunk, directory);
  registerInitTools(server, thunk, directory);
  registerHandoffTools(server, thunk);
  registerPingTools(server, thunk);
  registerDcpTools(server, thunk);
  registerExecutionInsightsTools(server, thunk);
  registerDoctrineTools(server, thunk);
  registerBriefTools(server, thunk);
  registerConfigTools(server, thunk);
  registerVisualTools(server, thunk);
  registerDoctorTools(server, thunk);
  registerHistoryTools(server, thunk);
  registerWorkflowTools(server, thunk);

  // Conditional: only register graph/search tools when available + not denied
  if (toolbox.isAvailable('bv')) registerGraphTools(server, thunk);
  if (toolbox.isAvailable('cass')) registerSearchTools(server, thunk);

  return server;
}

export async function main() {
  const directory = process.env.CLAUDE_PROJECT_DIR || process.cwd();
  const server = createMaestroServer(directory);
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

// Only auto-start when run directly, not when imported by start.mjs
const isBunDirect = typeof Bun !== 'undefined' && Bun.main === Bun.resolveSync(import.meta.path, '.');
if (isBunDirect) {
  main().catch((err) => {
    console.error('[maestro] Server failed to start:', err);
    process.exit(1);
  });
}
