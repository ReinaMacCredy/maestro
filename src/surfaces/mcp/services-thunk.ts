/**
 * Lazy service initialization for the MCP server.
 * Defers initServices() until a tool is actually called,
 * allowing the server to start even if .maestro/ doesn't exist yet.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { createContainer, type MaestroContainer } from '../../container.ts';
import { MaestroError } from '../../domain/errors.ts';
import type { ToolboxRegistry } from '../../infra/toolbox/registry.ts';
import type { WorkflowRegistry } from '../../app/workflow/registry.ts';

export type MaestroServices = MaestroContainer;

export interface ServicesThunk {
  /** Get or initialize services. Throws if .maestro/ is missing. */
  get(): MaestroContainer;
  /** Check if services have been initialized. */
  isInitialized(): boolean;
  /** Force initialization (used by maestro_init after creating .maestro/). */
  forceInit(): MaestroContainer;
  /** Invalidate cached container so next get() rebuilds it from current settings. */
  invalidate(): void;
}

export function createServicesThunk(
  directory: string,
  toolbox?: ToolboxRegistry,
  workflowRegistry?: WorkflowRegistry,
): ServicesThunk {
  let cached: MaestroContainer | null = null;

  function init(): MaestroContainer {
    const container = createContainer(directory, toolbox);
    if (workflowRegistry) {
      // Inject workflow registry into frozen container via property override
      return Object.freeze({ ...container, workflowRegistry });
    }
    return container;
  }

  return {
    get(): MaestroContainer {
      if (cached) return cached;

      const maestroDir = path.join(directory, '.maestro');
      if (!fs.existsSync(maestroDir)) {
        throw new MaestroError(
          'No .maestro/ directory found in this project',
          ['Run maestro_init first to set up this project for maestro orchestration'],
        );
      }

      cached = init();
      return cached;
    },

    isInitialized(): boolean {
      return cached !== null;
    },

    forceInit(): MaestroContainer {
      cached = init();
      return cached;
    },

    invalidate(): void {
      cached = null;
    },
  };
}
