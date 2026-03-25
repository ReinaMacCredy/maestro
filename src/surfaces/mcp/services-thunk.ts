/**
 * Lazy service initialization for the MCP server.
 * Defers initServices() until a tool is actually called,
 * allowing the server to start even if .maestro/ doesn't exist yet.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { initServices, type MaestroServices } from '../../services.ts';
import { MaestroError } from '../../core/errors.ts';
import type { ToolboxRegistry } from '../../toolbox/registry.ts';
import type { WorkflowRegistry } from '../../workflow/registry.ts';

export interface ServicesThunk {
  /** Get or initialize services. Throws if .maestro/ is missing. */
  get(): MaestroServices;
  /** Check if services have been initialized. */
  isInitialized(): boolean;
  /** Force initialization (used by maestro_init after creating .maestro/). */
  forceInit(): MaestroServices;
}

export function createServicesThunk(
  directory: string,
  toolbox?: ToolboxRegistry,
  workflowRegistry?: WorkflowRegistry,
): ServicesThunk {
  let cached: MaestroServices | null = null;

  function inject(services: MaestroServices): MaestroServices {
    if (workflowRegistry) services.workflowRegistry = workflowRegistry;
    return services;
  }

  return {
    get(): MaestroServices {
      if (cached) return cached;

      const maestroDir = path.join(directory, '.maestro');
      if (!fs.existsSync(maestroDir)) {
        throw new MaestroError(
          'No .maestro/ directory found in this project',
          ['Run maestro_init first to set up this project for maestro orchestration'],
        );
      }

      cached = inject(initServices(directory, toolbox));
      return cached;
    },

    isInitialized(): boolean {
      return cached !== null;
    },

    forceInit(): MaestroServices {
      cached = inject(initServices(directory, toolbox));
      return cached;
    },
  };
}
