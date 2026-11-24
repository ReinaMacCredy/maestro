import { failureEnvelope } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { briefDeliveredBy, type BriefService } from "./coordination.ts";
import { readPackageVersion } from "./version.ts";

type JsonRpcId = number | string | null;

interface JsonRpcRequest {
  id?: JsonRpcId;
  jsonrpc?: string;
  method?: string;
  params?: Record<string, unknown>;
}

function writeResponse(id: JsonRpcId, result: unknown): void {
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result })}\n`);
}

function writeError(id: JsonRpcId, code: number, message: string): void {
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, error: { code, message } })}\n`);
}

async function handleRequest(context: PluginContext, request: JsonRpcRequest): Promise<void> {
  context.sessions.refresh();
  const hasId = Object.prototype.hasOwnProperty.call(request, "id");
  const id = request.id ?? null;
  if (request.jsonrpc !== "2.0" || typeof request.method !== "string") {
    if (hasId) writeError(id, -32600, "invalid JSON-RPC request");
    return;
  }
  if (request.method === "notifications/initialized") return;
  if (!hasId) return;
  if (request.method === "initialize") {
    const session = context.sessions.record("mcp.initialize");
    const brief = context.brief as BriefService;
    const spoken = briefDeliveredBy(context, "hook.record", session);
    if (!spoken && !context.store.readOnly) {
      context.log.append({
        type: "mcp.initialize",
        entityType: "session",
        entityId: session.id,
        sessionId: session.id,
        payload: { pid: session.pid },
      });
    }
    writeResponse(id, {
      protocolVersion: "2025-06-18",
      capabilities: { tools: {} },
      serverInfo: { name: "maestro", version: await readPackageVersion() },
      instructions: spoken ? "" : await brief.render(session.id),
    });
    return;
  }
  if (request.method === "tools/list") {
    writeResponse(id, { tools: [] });
    return;
  }
  writeError(id, -32601, `method not found: ${request.method}`);
}

async function handleLine(context: PluginContext, line: string): Promise<void> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(line) as unknown;
  } catch {
    writeError(null, -32700, "parse error");
    return;
  }
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    writeError(null, -32600, "invalid JSON-RPC request");
    return;
  }
  const request = parsed as JsonRpcRequest;
  try {
    await handleRequest(context, request);
  } catch (error) {
    const envelope = failureEnvelope(error);
    const id = typeof request.id === "string" || typeof request.id === "number"
      ? request.id
      : null;
    writeError(id, -32603, envelope.error.message);
  }
}

async function serve(context: PluginContext): Promise<void> {
  const reader = Bun.stdin.stream().getReader();
  const decoder = new TextDecoder();
  let buffered = "";
  while (true) {
    const chunk = await reader.read();
    if (chunk.done) break;
    buffered += decoder.decode(chunk.value, { stream: true });
    while (buffered.includes("\n")) {
      const newline = buffered.indexOf("\n");
      const line = buffered.slice(0, newline).trimEnd();
      buffered = buffered.slice(newline + 1);
      if (!line) continue;
      await handleLine(context, line);
    }
  }
  const final = `${buffered}${decoder.decode()}`.trim();
  if (final) await handleLine(context, final);
}

export const mcpPlugin: BuiltInPlugin = {
  name: "mcp",
  inject: ["brief"],
  apply(context) {
    context.effect(() =>
      context.cli.register(
        "mcp serve",
        async () => {
          await serve(context);
        },
        {
          description: "Serve the session brief as MCP initialize instructions over foreground stdio; no tools.",
          mutates: false,
          rootDescription: "Deliver the session brief through a foreground stdio MCP transport.",
        },
      ),
    );
  },
};
