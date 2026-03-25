/**
 * Brief tool folded into maestro_task_read (what: brief).
 * This file is kept for backward compatibility of imports; no tools registered here.
 */

import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';

// No-op: brief functionality is now handled by maestro_task_read with what: brief
export function registerBriefTools(_server: McpServer, _thunk: ServicesThunk): void {
  // Brief is registered as part of maestro_task_read in task.ts
}
