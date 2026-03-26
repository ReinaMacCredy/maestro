/**
 * MCP tools for session history search via CASS.
 */

import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, errorResponse, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY } from '../annotations.ts';
import { limitParam } from '../params.ts';
import { requireSearchPort } from '../../../infra/utils/resolve.ts';

export function registerSearchTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_search',
    {
      description:
        'Search past agent sessions to find relevant prior work. Use when you need context from earlier sessions, ' +
        'want to know what was previously done on a file, or need to find similar past work. Requires CASS integration. ' +
        'What: sessions (requires: query), related (requires: file_path), similar (requires: content). ' +
        'Example: maestro_search({ what: "sessions", query: "auth refactor" })',
      inputSchema: {
        what: z.enum(['sessions', 'related', 'similar']).optional().describe('Query to perform'),
        action: z.enum(['sessions', 'related', 'similar']).optional().describe('(deprecated, use what)'),
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
      const what = input.what ?? input.action;
      if (!what) return errorResponse({ terminal: false, reason: 'validation', error: 'what is required' });
      switch (what) {
        case 'sessions': {
          if (!input.query) return errorResponse({ terminal: false, reason: 'validation', error: 'query is required for action: sessions', suggestions: ['Provide the query parameter.'] });
          const results = await port.searchSessions(input.query, {
            agent: input.agent,
            limit: input.limit,
            days: input.days,
          });
          return respond({ results });
        }
        case 'related': {
          if (!input.file_path) return errorResponse({ terminal: false, reason: 'validation', error: 'file_path is required for action: related', suggestions: ['Provide the file_path parameter.'] });
          const results = await port.findRelatedSessions(input.file_path, input.limit);
          return respond({ results });
        }
        case 'similar': {
          if (!input.content) return errorResponse({ terminal: false, reason: 'validation', error: 'content is required for action: similar', suggestions: ['Provide the content parameter.'] });
          const results = await port.searchSimilar(input.content, { limit: input.limit });
          return respond({ results });
        }
        default:
          return errorResponse({ terminal: true, reason: 'unknown_action', error: `Unknown action: ${what}` });
      }
    }),
  );
}
