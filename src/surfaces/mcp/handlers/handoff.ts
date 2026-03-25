/**
 * MCP tools for cross-agent handoff via Agent Mail.
 */

import { z } from 'zod';
import * as fs from 'node:fs';
import * as path from 'node:path';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY, ANNOTATIONS_MUTATING } from '../annotations.ts';
import { requireFeature, resolveFeature, requireHandoffPort } from '../../../core/resolve.ts';
import { featureParam } from '../params.ts';
import { buildAndSendHandoff } from '../../../handoff/usecases.ts';
import { getHandoffsPath, getHandoffPath } from '../../../core/paths.ts';
import { readText, fileExists } from '../../../core/fs-io.ts';

export function registerHandoffTools(server: McpServer, thunk: ServicesThunk): void {
  // Mutating: send | ack
  server.registerTool(
    'maestro_handoff',
    {
      description:
        'Handoff mutations via Agent Mail. Actions: send (send handoff document to another agent), ' +
        'ack (acknowledge receipt of a handoff message).',
      inputSchema: {
        action: z.enum(['send', 'ack']).describe('Action to perform'),
        feature: featureParam(),
        task: z.string().optional().describe('Task/bead ID or folder name (required for send)'),
        target_agent: z.string().optional().describe('Target agent name (omit for broadcast, send only)'),
        additional_context: z.string().optional().describe('Extra context to include (send only)'),
        thread_id: z.string().optional().describe('Thread ID to acknowledge (required for ack)'),
      },
      annotations: ANNOTATIONS_MUTATING,
    },
    withErrorHandling(async (input) => {
      switch (input.action) {
        case 'send': {
          if (!input.task) return respond({ error: 'task is required for action: send' });
          const port = requireHandoffPort(thunk.get());
          const services = thunk.get();
          const feature = requireFeature(services, input.feature);
          const { result } = await buildAndSendHandoff(port, feature, input.task, {
            targetAgent: input.target_agent,
            additionalContext: input.additional_context,
          });
          return respond({ feature, task: input.task, ...result });
        }
        case 'ack': {
          if (!input.thread_id) return respond({ error: 'thread_id is required for action: ack' });
          const port = requireHandoffPort(thunk.get());
          await port.acknowledgeHandoff(input.thread_id);
          return respond({ threadId: input.thread_id });
        }
        default:
          return respond({ error: `Unknown action: ${(input as { action: string }).action}` });
      }
    }),
  );

  // Read-only: read | list | status | receive
  server.registerTool(
    'maestro_handoff_read',
    {
      description:
        'Handoff read operations. What: read (specific handoff file content), list (all handoffs for feature), ' +
        'status (exists + acknowledged check), receive (check Agent Mail for pending handoffs).',
      inputSchema: {
        what: z.enum(['read', 'list', 'status', 'receive']).describe('What to read'),
        feature: featureParam(),
        id: z.string().optional().describe('Handoff ID (required for read, status)'),
        agent_id: z.string().optional().describe('Your agent name/ID (required for receive)'),
      },
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async (input) => {
      switch (input.what) {
        case 'read': {
          if (!input.id) return respond({ error: 'id is required for what: read' });
          const services = thunk.get();
          const feature = requireFeature(services, input.feature);
          const filePath = getHandoffPath(services.directory, feature, input.id);
          const content = readText(filePath);
          if (content === null) {
            throw new Error(`Handoff not found: ${input.id} (looked at ${filePath})`);
          }
          return respond({ feature, id: input.id, filePath, content });
        }
        case 'list': {
          const services = thunk.get();
          const feature = requireFeature(services, input.feature);
          const handoffsDir = getHandoffsPath(services.directory, feature);
          const entries: Array<{ id: string; filePath: string; createdAt: string; acknowledged: boolean }> = [];
          try {
            const files = fs.readdirSync(handoffsDir).filter(f => f.endsWith('.md'));
            for (const file of files) {
              const filePath = path.join(handoffsDir, file);
              const stat = fs.statSync(filePath);
              const id = file.replace(/\.md$/, '');
              const ackPath = `${filePath}.ack`;
              entries.push({
                id,
                filePath,
                createdAt: stat.mtime.toISOString(),
                acknowledged: fileExists(ackPath),
              });
            }
          } catch {
            // No handoffs directory yet
          }
          return respond({ feature, handoffs: entries, count: entries.length });
        }
        case 'status': {
          if (!input.id) return respond({ error: 'id is required for what: status' });
          const services = thunk.get();
          const feature = requireFeature(services, input.feature);
          const filePath = getHandoffPath(services.directory, feature, input.id);
          let exists = false;
          let createdAt: string | undefined;
          try {
            const stat = fs.statSync(filePath);
            exists = true;
            createdAt = stat.mtime.toISOString();
          } catch { /* file does not exist */ }
          const acknowledged = exists && fileExists(`${filePath}.ack`);
          return respond({ feature, id: input.id, exists, acknowledged, filePath, createdAt });
        }
        case 'receive': {
          if (!input.agent_id) return respond({ error: 'agent_id is required for what: receive' });
          const port = requireHandoffPort(thunk.get());
          const services = thunk.get();
          const feature = resolveFeature(services, input.feature);
          const handoffs = await port.receiveHandoffs(feature ?? undefined, input.agent_id);
          return respond({ handoffs });
        }
        default:
          return respond({ error: `Unknown what: ${(input as { what: string }).what}` });
      }
    }),
  );
}
