import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY } from '../annotations.ts';
import { doctor } from '../../../app/workflow/doctor.ts';

export function registerDoctorTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_doctor',
    {
      description: 'Diagnose maestro configuration issues. Use when something seems misconfigured or broken -- validates config, checks active feature, task backend health, and integration availability. ' +
        'Example: maestro_doctor() ' +
        'Returns: { checks[], healthy, summary }',
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async () => {
      const services = thunk.get();
      const report = await doctor(services);
      return respond({ ...report });
    }),
  );
}
