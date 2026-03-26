import * as path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_MUTATING } from '../annotations.ts';
import { getMaestroPath } from '../../../infra/utils/paths.ts';
import { ensureDir } from '../../../infra/utils/fs-io.ts';
import { MaestroError } from '../../../domain/errors.ts';

const execFileAsync = promisify(execFile);

export function registerInitTools(server: McpServer, thunk: ServicesThunk, directory?: string): void {
  server.registerTool(
    'maestro_init',
    {
      description: 'Initialize maestro for a new project. Use when setting up maestro in a repo for the first time. Creates .maestro/ directory structure and default configuration.',
      inputSchema: {
        // z.object({}) not needed -- empty schema means no inputs
      },
      annotations: ANNOTATIONS_MUTATING,
    },
    withErrorHandling(async (_input) => {
      const dir = directory;
      if (!dir) {
        throw new MaestroError('No project directory configured', []);
      }

      const maestroPath = getMaestroPath(dir);
      ensureDir(maestroPath);
      ensureDir(path.join(maestroPath, 'features'));

      let brInitialized = false;
      try {
        await execFileAsync('br', ['init'], { cwd: dir });
        brInitialized = true;
      } catch {
        // br not available or init failed -- non-fatal
      }

      thunk.forceInit();

      return respond({
        projectRoot: dir,
        maestroPath,
        brInitialized,
      });
    }),
  );
}
