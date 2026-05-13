import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { fail, toCallToolResult, type CallToolResult } from "./errors.js";

type RequestHandler = (request: unknown, extra: unknown) => Promise<unknown>;

interface PrivateProtocol {
  _requestHandlers: Map<string, RequestHandler>;
}

interface PrivateServerHolder {
  server: PrivateProtocol;
}

/**
 * Replaces the SDK's `tools/call` handler with a wrapper that rewrites
 * the SDK's verbose InvalidParams error text into the doctrine shape:
 * `{ code: "INVALID_ARG", message, arg }`.
 *
 * Call after all tools are registered so the SDK has installed its handler.
 */
export function installToolErrorInterceptor(server: McpServer): void {
  const holder = server as unknown as PrivateServerHolder;
  const handlers = holder.server._requestHandlers;
  const original = handlers.get("tools/call");
  if (original === undefined) return;
  handlers.set("tools/call", async (request, extra) => {
    const result = await original(request, extra);
    return rewriteInvalidParamsError(result);
  });
}

const INPUT_VALIDATION_PREFIX = "MCP error -32602: Input validation error:";

function rewriteInvalidParamsError(result: unknown): unknown {
  if (!isInvalidParamsResult(result)) return result;
  const text = result.content[0].text;
  const stripped = text.slice(INPUT_VALIDATION_PREFIX.length).trim();
  const issues = extractZodIssues(stripped);
  const first = issues[0];
  if (first === undefined) {
    return toCallToolResult(fail("INVALID_ARG", stripped));
  }
  const arg = pathToArg(first.path);
  const message = first.message ?? "Invalid argument";
  return toCallToolResult(fail("INVALID_ARG", message, { arg }));
}

interface CallResultLike {
  content: Array<{ type: string; text: string }>;
  isError?: boolean;
}

function isInvalidParamsResult(value: unknown): value is CallResultLike {
  if (value === null || typeof value !== "object") return false;
  const v = value as Record<string, unknown>;
  if (v.isError !== true) return false;
  const content = v.content;
  if (!Array.isArray(content) || content.length === 0) return false;
  const first = content[0] as Record<string, unknown> | undefined;
  if (!first || first.type !== "text" || typeof first.text !== "string") return false;
  return first.text.startsWith(INPUT_VALIDATION_PREFIX);
}

interface ZodIssueLike {
  path?: ReadonlyArray<string | number>;
  message?: string;
}

function extractZodIssues(text: string): ZodIssueLike[] {
  const start = text.indexOf("[");
  if (start === -1) return [];
  const slice = text.slice(start);
  try {
    const parsed = JSON.parse(slice) as unknown;
    if (Array.isArray(parsed)) return parsed as ZodIssueLike[];
  } catch {
    // Some SDK paths embed Zod's "tool args:\n[...]" prefix or extra noise.
    // Fall through and try a salvage parse on the tail.
  }
  return [];
}

function pathToArg(path: ReadonlyArray<string | number> | undefined): string | undefined {
  if (!path || path.length === 0) return undefined;
  return path.map((segment) => String(segment)).join(".");
}

export type { CallToolResult };
