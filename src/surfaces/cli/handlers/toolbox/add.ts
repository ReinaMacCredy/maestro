/**
 * maestro toolbox-add -- quick-add a tool (manifest only, no adapter).
 * For MCP tools that will use auto-bridge, or CLI tools with manual adapter later.
 */

import * as fs from 'fs';
import * as path from 'path';
import { defineCommand } from 'citty';
import { output } from '../../../../infra/utils/output.ts';
import { MaestroError, handleCommandError } from '../../../../domain/errors.ts';
import type { TransportType } from '../../../../infra/toolbox/sdk/types.ts';

const VALID_TRANSPORTS: TransportType[] = ['cli', 'http', 'mcp-stdio', 'mcp-http'];

export default defineCommand({
  meta: { name: 'toolbox-add', description: 'Quick-add a tool (manifest only, no adapter)\n\nExamples:\n  maestro toolbox-add --name my-tool --transport cli --binary my-tool\n  maestro toolbox-add --name my-mcp --transport mcp-stdio --command "npx my-mcp"' },
  args: {
    name: {
      type: 'string',
      description: 'Tool name (kebab-case)',
      required: true,
    },
    transport: {
      type: 'string',
      description: 'Transport type: cli, http, mcp-stdio, mcp-http',
      required: true,
    },
    binary: {
      type: 'string',
      description: 'Binary name (for cli transport)',
    },
    command: {
      type: 'string',
      description: 'Launch command (for mcp-stdio transport)',
    },
    url: {
      type: 'string',
      description: 'Server URL (for mcp-http or http transport)',
    },
    provides: {
      type: 'string',
      description: 'Port name this tool provides',
    },
  },
  async run({ args }) {
    try {
      const transport = args.transport as TransportType;
      if (!VALID_TRANSPORTS.includes(transport)) {
        throw new MaestroError(
          `Invalid transport: ${args.transport}`,
          [`Valid transports: ${VALID_TRANSPORTS.join(', ')}`],
        );
      }

      if (!/^[a-z][a-z0-9-]*$/.test(args.name)) {
        throw new MaestroError('Tool name must be kebab-case');
      }

      const toolDir = path.join(import.meta.dir, '../../../../infra/toolbox/tools/external', args.name);
      if (fs.existsSync(path.join(toolDir, 'manifest.json'))) {
        throw new MaestroError(`Tool '${args.name}' already has a manifest at ${toolDir}`);
      }

      const manifest: Record<string, unknown> = {
        name: args.name,
        transport,
        provides: args.provides ?? null,
        priority: 100,
        adapter: `tools/external/${args.name}/adapter.ts`,
      };

      if (transport === 'cli') {
        manifest.binary = args.binary ?? args.name;
        manifest.detect = `${manifest.binary} --version`;
      } else {
        manifest.binary = null;
        manifest.detect = null;
      }

      if (transport === 'mcp-stdio' && args.command) {
        manifest.command = args.command;
      }
      if ((transport === 'mcp-http' || transport === 'http') && args.url) {
        if (transport === 'mcp-http') manifest.url = args.url;
        else manifest.baseUrl = args.url;
      }

      fs.mkdirSync(toolDir, { recursive: true });
      fs.writeFileSync(path.join(toolDir, 'manifest.json'), JSON.stringify(manifest, null, 2));

      output(
        { name: args.name, transport, path: toolDir },
        () => `[ok] Added tool '${args.name}' (manifest only) at ${toolDir}`,
      );
    } catch (err) {
      handleCommandError('toolbox-add', err);
    }
  },
});
