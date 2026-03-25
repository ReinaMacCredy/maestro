import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY } from '../annotations.ts';
import { ping } from '../../../workflow/ping.ts';

export function registerPingTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_ping',
    {
      description: 'Health check: version, backend, integrations. No feature context needed.',
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async () => {
      const services = thunk.get();
      const result = ping(services);
      return respond({ ...result });
    }),
  );
}
