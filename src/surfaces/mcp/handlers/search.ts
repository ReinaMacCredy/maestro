/**
 * MCP tools for session history search via CASS.
 */

import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY } from '../annotations.ts';
import { limitParam } from '../params.ts';
import { requireSearchPort } from '../../../core/resolve.ts';

export function registerSearchTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_search',
    {
      description:
        'Session history search. Actions: sessions (full-text search of past agent sessions), ' +
        'related (find sessions that worked on a specific file), similar (find sessions with similar content).',
      inputSchema: {
        action: z.enum(['sessions', 'related', 'similar']).describe('Action to perform'),
        query: z.string().optional().describe('Search query (required for sessions)'),
        agent: z.string().optional().describe('Filter to specific agent -- claude, codex, cursor, etc. (sessions only)'),
        limit: limitParam(10),
        days: z.number().optional().describe('Limit to recent N days (sessions only)'),
        file_path: z.string().optional().describe('File path to search for (required for related)'),
        content: z.string().optional().describe('Content text to find similar sessions for (required for similar)'),
      },
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async (input) => {
      const port = requireSearchPort(thunk.get());
      switch (input.action) {
        case 'sessions': {
          if (!input.query) return respond({ error: 'query is required for action: sessions' });
          const results = await port.searchSessions(input.query, {
            agent: input.agent,
            limit: input.limit,
            days: input.days,
          });
          return respond({ results });
        }
        case 'related': {
          if (!input.file_path) return respond({ error: 'file_path is required for action: related' });
          const results = await port.findRelatedSessions(input.file_path, input.limit);
          return respond({ results });
        }
        case 'similar': {
          if (!input.content) return respond({ error: 'content is required for action: similar' });
          const results = await port.searchSimilar(input.content, { limit: input.limit });
          return respond({ results });
        }
        default:
          return respond({ error: `Unknown action: ${(input as { action: string }).action}` });
      }
    }),
  );
}
