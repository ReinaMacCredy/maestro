import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY } from '../annotations.ts';
import { ping } from '../../../app/workflow/ping.ts';

export function registerPingTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_ping',
    {
      description: 'Quick health check returning maestro version, active backend, and available integrations. Use to verify maestro is running and connected. ' +
        'Example: maestro_ping() ' +
        'Returns: { version, backend, integrations, directory }',
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async () => {
      const services = thunk.get();
      const result = ping(services);
      return respond({ ...result });
    }),
  );
}
