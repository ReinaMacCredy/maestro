import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY } from '../annotations.ts';
import { doctor } from '../../../workflow/doctor.ts';

export function registerDoctorTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_doctor',
    {
      description: 'Health check: config validation, active feature, task backend, and integration availability.',
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async () => {
      const services = thunk.get();
      const report = await doctor(services);
      return respond({ ...report });
    }),
  );
}
