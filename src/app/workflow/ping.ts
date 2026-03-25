/**
 * ping use case.
 * Returns version, project root, task backend, and integration availability.
 */

import type { GraphPort } from '../../tasks/graph/port.ts';
import type { HandoffPort } from '../../handoff/port.ts';
import type { SearchPort } from '../../search/port.ts';
import type { AgentToolsRegistry } from '../../toolbox/agents/registry.ts';
import { VERSION } from '../../version.ts';

export interface PingServices {
  directory: string;
  taskBackend: 'fs' | 'br';
  agentToolsRegistry: AgentToolsRegistry;
  graphPort?: GraphPort;
  handoffPort?: HandoffPort;
  searchPort?: SearchPort;
}

export interface PingResult {
  version: string;
  projectRoot: string;
  taskBackend: string;
  integrations: {
    br: boolean;
    bv: boolean;
    cass: boolean;
    agentMail: boolean;
  };
  agentTools: {
    installed: number;
    total: number;
    names: string[];
  };
}

export function ping(services: PingServices): PingResult {
  const agentAll = services.agentToolsRegistry.getAll();
  const agentInstalled = services.agentToolsRegistry.getInstalled();

  return {
    version: VERSION,
    projectRoot: services.directory,
    taskBackend: services.taskBackend,
    integrations: {
      br: services.taskBackend === 'br',
      bv: !!services.graphPort,
      cass: !!services.searchPort,
      agentMail: !!services.handoffPort,
    },
    agentTools: {
      installed: agentInstalled.length,
      total: agentAll.length,
      names: agentInstalled.map(a => a.manifest.name),
    },
  };
}
