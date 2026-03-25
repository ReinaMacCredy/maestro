/**
 * MCP client transport (stdio + HTTP unified).
 * Wraps the official @modelcontextprotocol/sdk Client.
 */

import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';
import type { McpStdioTransportConfig, McpHttpTransportConfig } from './types.ts';

export interface McpToolResult {
  content: Array<{ type: string; text?: string }>;
  isError?: boolean;
}

export interface McpResource {
  uri: string;
  name?: string;
  mimeType?: string;
  text?: string;
}

export class McpTransport {
  private client: Client;
  private transport: StdioClientTransport | StreamableHTTPClientTransport;
  private connected = false;

  private constructor(
    client: Client,
    transport: StdioClientTransport | StreamableHTTPClientTransport,
  ) {
    this.client = client;
    this.transport = transport;
  }

  /**
   * Create an MCP transport over stdio subprocess.
   */
  static fromStdio(config: McpStdioTransportConfig): McpTransport {
    const transport = new StdioClientTransport({
      command: config.command,
      args: config.args,
      env: config.env ? { ...Object.fromEntries(Object.entries(process.env).filter((e): e is [string, string] => e[1] !== undefined)), ...config.env } : undefined,
      cwd: config.cwd,
    });
    const client = new Client({ name: 'maestro', version: '1.0.0' });
    return new McpTransport(client, transport);
  }

  /**
   * Create an MCP transport over HTTP (Streamable HTTP).
   */
  static fromHttp(config: McpHttpTransportConfig): McpTransport {
    const transport = new StreamableHTTPClientTransport(
      new URL(config.url),
      { requestInit: config.authHeaders ? { headers: config.authHeaders } : undefined },
    );
    const client = new Client({ name: 'maestro', version: '1.0.0' });
    return new McpTransport(client, transport);
  }

  /** Lazy connect on first call. */
  private async ensureConnected(): Promise<void> {
    if (this.connected) return;
    await this.client.connect(this.transport);
    this.connected = true;
  }

  async callTool(name: string, args: Record<string, unknown> = {}): Promise<McpToolResult> {
    await this.ensureConnected();
    const result = await this.client.callTool({ name, arguments: args });
    return result as McpToolResult;
  }

  async readResource(uri: string): Promise<McpResource | null> {
    await this.ensureConnected();
    const result = await this.client.readResource({ uri });
    const content = result.contents?.[0];
    if (!content) return null;
    return {
      uri: content.uri,
      mimeType: content.mimeType,
      text: 'text' in content ? (content.text as string) : undefined,
    };
  }

  async listTools(): Promise<Array<{ name: string; description?: string }>> {
    await this.ensureConnected();
    const result = await this.client.listTools();
    return result.tools.map(t => ({ name: t.name, description: t.description }));
  }

  async close(): Promise<void> {
    if (!this.connected) return;
    try {
      await this.client.close();
    } catch {
      // Best-effort cleanup
    }
    this.connected = false;
  }
}
