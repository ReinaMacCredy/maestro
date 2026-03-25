/**
 * Generic MCP bridge adapter factory.
 * Creates a port-compatible proxy from manifest config + tool mappings.
 */

import { McpTransport } from './mcp-transport.ts';
import { McpBridge, extractJson, extractText } from './mcp-bridge.ts';
import type { McpToolResult } from './mcp-transport.ts';
import type { AdapterContext } from '../types.ts';

export interface BridgeMapping {
  /** MCP tool name to call. */
  mcpTool: string;
  /** Port method name this maps to. */
  portMethod: string;
  /** Transform MCP result to port return type. */
  transform: (result: McpToolResult) => unknown;
  /** Convert port method args to MCP tool args. Default: first arg as-is. */
  args?: (...portArgs: unknown[]) => Record<string, unknown>;
}

/**
 * Create a port adapter backed by an MCP server via McpBridge.
 *
 * Returns a Proxy object that routes method calls to the appropriate
 * MCP tool through the bridge's reverse mapping.
 */
export function createMcpPortAdapter<T extends object>(
  ctx: AdapterContext,
  mappings: BridgeMapping[],
): T {
  const manifest = ctx.manifest;

  let transport: McpTransport;
  if (manifest.transport === 'mcp-stdio') {
    transport = McpTransport.fromStdio({
      command: manifest.command!,
      args: manifest.args,
      env: manifest.env,
      cwd: ctx.projectRoot,
    });
  } else if (manifest.transport === 'mcp-http') {
    transport = McpTransport.fromHttp({
      url: manifest.url!,
    });
  } else {
    throw new Error(`Cannot create MCP bridge for transport: ${manifest.transport}`);
  }

  const bridge = new McpBridge(transport);
  const argConverters = new Map<string, (...args: unknown[]) => Record<string, unknown>>();

  for (const m of mappings) {
    bridge.mapTool(m.mcpTool, m.transform, m.portMethod);
    if (m.args) {
      argConverters.set(m.portMethod, m.args);
    }
  }

  return new Proxy({} as T, {
    get(_target, prop: string) {
      if (prop === 'close' || prop === 'dispose') {
        return () => bridge.close();
      }
      const mcpTool = (bridge as any).reverseMap?.get(prop);
      if (!mcpTool) return undefined;

      return async (...args: unknown[]) => {
        const converter = argConverters.get(prop);
        const mcpArgs = converter ? converter(...args) : (typeof args[0] === 'object' && args[0] !== null ? args[0] as Record<string, unknown> : {});
        return bridge.callByMethod(prop, mcpArgs);
      };
    },
  });
}

export { extractJson, extractText };
