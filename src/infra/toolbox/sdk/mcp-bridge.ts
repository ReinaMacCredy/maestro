/**
 * MCP Bridge -- maps MCP tool names to port method calls.
 * Production-ready with JSON extraction helpers and error context.
 */

import type { McpTransport, McpToolResult } from './mcp-transport.ts';

export type ToolMapping<T> = (result: McpToolResult) => T;

// ============================================================================
// Extraction Helpers
// ============================================================================

/** Safely extract text from the first content item of an MCP tool result. */
export function extractText(result: McpToolResult): string {
  return result.content?.[0]?.text ?? '';
}

/** Extract and parse JSON from the first content item. Throws on parse failure. */
export function extractJson<T>(result: McpToolResult): T {
  const text = extractText(result);
  if (!text) throw new Error('MCP tool returned empty content');
  return JSON.parse(text) as T;
}

// ============================================================================
// Port Method Registry (for discovery suggestions)
// ============================================================================

const PORT_METHODS: Record<string, string[]> = {
  search: ['searchSessions', 'findRelatedSessions'],
  graph: ['getInsights', 'getNextRecommendation', 'getExecutionPlan'],
  tasks: ['create', 'get', 'list', 'claim', 'done', 'block', 'unblock'],
  handoff: ['buildHandoff', 'sendHandoff', 'receiveHandoffs', 'acknowledgeHandoff'],
};

export interface SuggestedMapping {
  mcpTool: string;
  portMethod: string;
  confidence: number;
}

// ============================================================================
// McpBridge
// ============================================================================

/**
 * McpBridge auto-bridges MCP tools to port method calls.
 * Register mappings with mapTool(), then call tools with call().
 */
export class McpBridge {
  private transport: McpTransport;
  private mappings: Map<string, ToolMapping<unknown>> = new Map();
  private reverseMap: Map<string, string> = new Map();

  constructor(transport: McpTransport) {
    this.transport = transport;
  }

  /**
   * Register a mapping from MCP tool name to a result transformer.
   */
  mapTool<T>(mcpToolName: string, transform: ToolMapping<T>, portMethodName?: string): this {
    this.mappings.set(mcpToolName, transform as ToolMapping<unknown>);
    if (portMethodName) {
      this.reverseMap.set(portMethodName, mcpToolName);
    }
    return this;
  }

  /**
   * Call an MCP tool and transform the result.
   * Wraps transform errors with tool name context.
   */
  async call<T>(mcpToolName: string, args: Record<string, unknown> = {}): Promise<T> {
    const transform = this.mappings.get(mcpToolName);
    if (!transform) {
      throw new Error(`No mapping registered for MCP tool: ${mcpToolName}`);
    }
    const result = await this.transport.callTool(mcpToolName, args);
    if (result.isError) {
      throw new Error(`MCP tool '${mcpToolName}' returned error: ${extractText(result)}`);
    }
    try {
      return transform(result) as T;
    } catch (e) {
      throw new Error(
        `Transform failed for MCP tool '${mcpToolName}': ${e instanceof Error ? e.message : String(e)}`,
      );
    }
  }

  /**
   * Call by port method name (reverse lookup).
   */
  async callByMethod<T>(portMethodName: string, args: Record<string, unknown> = {}): Promise<T> {
    const mcpTool = this.reverseMap.get(portMethodName);
    if (!mcpTool) {
      throw new Error(`No MCP tool mapped for port method: ${portMethodName}`);
    }
    return this.call<T>(mcpTool, args);
  }

  /** Look up the MCP tool name for a given port method. */
  getToolForMethod(portMethod: string): string | undefined {
    return this.reverseMap.get(portMethod);
  }

  /** List all registered tool mappings. */
  getMappings(): string[] {
    return [...this.mappings.keys()];
  }

  /** Discover available tools from the MCP server. */
  async discoverTools(): Promise<Array<{ name: string; description?: string }>> {
    return this.transport.listTools();
  }

  /**
   * Discover MCP tools and suggest port method mappings based on name similarity.
   */
  async discoverAndSuggest(portName: string): Promise<SuggestedMapping[]> {
    const methods = PORT_METHODS[portName];
    if (!methods) return [];

    const tools = await this.discoverTools();
    const suggestions: SuggestedMapping[] = [];

    for (const tool of tools) {
      const toolNorm = tool.name.toLowerCase().replace(/[_-]/g, '');
      for (const method of methods) {
        const methodNorm = method.toLowerCase();
        // Simple substring match for suggestion
        if (toolNorm.includes(methodNorm) || methodNorm.includes(toolNorm)) {
          suggestions.push({ mcpTool: tool.name, portMethod: method, confidence: 0.8 });
        }
      }
    }

    return suggestions;
  }

  /** Close the underlying transport. */
  async close(): Promise<void> {
    return this.transport.close();
  }
}
