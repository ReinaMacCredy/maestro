import {
  CliError,
  failureEnvelope,
  successEnvelope,
  type CliCommandDescriptor,
} from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { BriefService } from "./coordination.ts";
import type { RecipeService } from "./recipe.ts";
import { readPackageVersion } from "./version.ts";

type JsonRpcId = number | string | null;

interface JsonRpcRequest {
  id?: JsonRpcId;
  jsonrpc?: string;
  method?: string;
  params?: Record<string, unknown>;
}

interface McpToolResult {
  content: Array<{ text: string; type: "text" }>;
  isError?: boolean;
  structuredContent: unknown;
}

const tools = [
  {
    name: "maestro_find",
    description:
      "Search the live Maestro verb registry and the recipe list by keyword. Returns up to 10 matching verbs (name, description, positionals, flags with their descriptions), ranked by keyword hits in the verb name and then in its metadata, and up to 5 matching recipes, each with the maestro recipe show command that prints it; a query with no hits returns empty lists and a hint. Use it to learn a verb's exact flags before calling maestro_run. It runs nothing and does not search the store (that is the search verb through maestro_run).",
    inputSchema: {
      type: "object",
      properties: {
        query: {
          type: "string",
          description:
            "One or more keywords, matched case-insensitively against verb names, descriptions and flag names, and against recipe names and descriptions.",
        },
      },
      required: ["query"],
      additionalProperties: false,
    },
  },
  {
    name: "maestro_run",
    description:
      "Run one Maestro verb through the same strict CLI dispatcher as the terminal. Returns the verb's {ok: true, data} JSON envelope; on any failure (an unknown verb or flag, a stray positional, a policy gate such as GATE_BLOCKED) the result is a tool error carrying the CLI's {ok: false, error} envelope, which names the unblocking command when there is one. Mutating verbs run for real; there is no dry run. The line is split with shell-style quoting but no shell runs, so pipes, redirects, globs and environment variables are unavailable, and mcp serve cannot be started from here.",
    inputSchema: {
      type: "object",
      properties: {
        line: {
          type: "string",
          description:
            "The verb and its arguments exactly as typed after the word maestro, without that word, on one line; quote an argument that contains spaces with single or double quotes, or escape with a backslash.",
        },
      },
      required: ["line"],
      additionalProperties: false,
    },
  },
] as const;

const verbResultLimit = 10;
const recipeResultLimit = 5;

function toolResult(value: unknown): McpToolResult {
  return {
    content: [{ type: "text", text: JSON.stringify(value) }],
    structuredContent: value,
  };
}

function toolError(error: unknown): McpToolResult {
  const envelope = failureEnvelope(error);
  return {
    content: [{ type: "text", text: JSON.stringify(envelope) }],
    structuredContent: envelope,
    isError: true,
  };
}

function requiredString(
  value: Record<string, unknown> | undefined,
  name: string,
): string {
  const field = value?.[name];
  if (typeof field !== "string" || field.trim().length === 0) {
    throw new CliError("MCP_INVALID_ARGUMENT", `missing MCP argument: ${name}`, { argument: name });
  }
  return field;
}

function tokens(query: string): string[] {
  return query.toLowerCase().split(/[^a-z0-9]+/).filter(Boolean);
}

function verbScore(verb: CliCommandDescriptor, queryTokens: string[]): number {
  const name = verb.name.toLowerCase();
  const metadata = [
    verb.description,
    ...verb.flags.flatMap((flag) => [flag.name, flag.description]),
  ].map((value) => value.toLowerCase());
  return queryTokens.reduce((score, token) => {
    if (name.includes(token)) return score + 2;
    return metadata.some((value) => value.includes(token)) ? score + 1 : score;
  }, 0);
}

function recipeScore(
  entry: { description: string; name: string },
  queryTokens: string[],
): number {
  const name = entry.name.toLowerCase();
  const description = entry.description.toLowerCase();
  return queryTokens.reduce((score, token) => {
    if (name.includes(token)) return score + 2;
    return description.includes(token) ? score + 1 : score;
  }, 0);
}

function find(context: PluginContext, query: string): unknown {
  const queryTokens = tokens(query);
  const verbs = context.cli.describeCommands()
    .map((verb, index) => ({ index, score: verbScore(verb, queryTokens), verb }))
    .filter((match) => match.score > 0)
    .sort((left, right) => right.score - left.score || left.index - right.index)
    .slice(0, verbResultLimit)
    .map(({ verb }) => verb);
  const recipe = context.recipe as RecipeService | undefined;
  const recipes = (recipe?.list() ?? [])
    .map((entry, index) => ({ index, score: recipeScore(entry, queryTokens), entry }))
    .filter((match) => match.score > 0)
    .sort((left, right) => right.score - left.score || left.index - right.index)
    .slice(0, recipeResultLimit)
    .map(({ entry: { name, description } }) => ({
      name,
      description,
      command: `maestro recipe show ${name}`,
    }));
  const hint = verbs.length === 0 && recipes.length === 0
    ? "Run maestro help for the full verb list, or try shorter single-word keywords."
    : undefined;
  return { query, verbs, recipes, ...(hint ? { hint } : {}) };
}

function parseVerbLine(line: string): string[] {
  if (/\r|\n/.test(line)) {
    throw new CliError("MCP_INVALID_ARGUMENT", "maestro_run accepts exactly one verb line");
  }
  const args: string[] = [];
  let current = "";
  let quote: "'" | '"' | null = null;
  let escaped = false;
  let started = false;
  for (const character of line) {
    if (escaped) {
      current += character;
      escaped = false;
      started = true;
      continue;
    }
    if (character === "\\" && quote !== "'") {
      escaped = true;
      started = true;
      continue;
    }
    if (quote) {
      if (character === quote) {
        quote = null;
      } else {
        current += character;
      }
      started = true;
      continue;
    }
    if (character === "'" || character === '"') {
      quote = character;
      started = true;
      continue;
    }
    if (/\s/.test(character)) {
      if (started) {
        args.push(current);
        current = "";
        started = false;
      }
      continue;
    }
    current += character;
    started = true;
  }
  if (escaped || quote) {
    throw new CliError("MCP_INVALID_ARGUMENT", "unterminated quote or escape in verb line");
  }
  if (started) args.push(current);
  if (args.length === 0) {
    throw new CliError("MCP_INVALID_ARGUMENT", "maestro_run requires a verb line");
  }
  return args;
}

function writeResponse(id: JsonRpcId, result: unknown): void {
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result })}\n`);
}

function writeError(id: JsonRpcId, code: number, message: string): void {
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, error: { code, message } })}\n`);
}

async function callTool(
  context: PluginContext,
  params: Record<string, unknown> | undefined,
): Promise<McpToolResult> {
  const name = params?.name;
  const args = params?.arguments;
  const input = args && typeof args === "object" && !Array.isArray(args)
    ? args as Record<string, unknown>
    : undefined;
  try {
    if (name === "maestro_find") {
      return toolResult(find(context, requiredString(input, "query")));
    }
    if (name === "maestro_run") {
      const line = requiredString(input, "line");
      const argv = parseVerbLine(line);
      if (argv[0] === "mcp" && argv[1] === "serve") {
        throw new CliError(
          "MCP_RECURSION",
          "mcp serve cannot be started from inside maestro_run; invoke another verb",
        );
      }
      return toolResult(successEnvelope(await context.cli.execute(argv)));
    }
    throw new CliError("MCP_TOOL_NOT_FOUND", `unknown MCP tool: ${String(name)}`, { tool: name });
  } catch (error) {
    return toolError(error);
  }
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
    writeResponse(id, {
      protocolVersion: "2025-06-18",
      capabilities: { tools: {} },
      serverInfo: { name: "maestro", version: await readPackageVersion() },
      instructions: await brief.render(session.id),
    });
    return;
  }
  if (request.method === "tools/list") {
    writeResponse(id, { tools });
    return;
  }
  if (request.method === "tools/call") {
    writeResponse(id, await callTool(context, request.params));
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
          description: "Serve exactly two Maestro meta-tools over foreground stdio MCP.",
          mutates: false,
          rootDescription: "Expose Maestro through a foreground stdio MCP transport.",
        },
      ),
    );
  },
};
