import { expect, test } from "bun:test";
import { join } from "node:path";
import { version } from "../package.json";
import { runCli, withFixture, type Fixture } from "./helpers.ts";

interface JsonRpcResponse {
  error?: { code: number; message: string };
  id: number | null;
  jsonrpc: string;
  result?: Record<string, unknown>;
}

const cli = join(import.meta.dir, "..", "bin", "maestro.ts");

function startMcp(fixture: Fixture) {
  const child = Bun.spawn([process.execPath, cli, "mcp", "serve"], {
    cwd: fixture.repo,
    env: {
      ...process.env,
      HOME: fixture.home,
      MAESTRO_SESSION_ID: "test-session",
      MAESTRO_SESSION_PID: String(process.pid),
    },
    stdin: "pipe",
    stdout: "pipe",
    stderr: "pipe",
  });
  const reader = child.stdout.getReader();
  const decoder = new TextDecoder();
  let buffered = "";

  async function readResponse(method: string): Promise<JsonRpcResponse> {
    while (!buffered.includes("\n")) {
      const chunk = await reader.read();
      if (chunk.done) throw new Error(`MCP server closed before replying to ${method}`);
      buffered += decoder.decode(chunk.value, { stream: true });
    }
    const newline = buffered.indexOf("\n");
    const line = buffered.slice(0, newline);
    buffered = buffered.slice(newline + 1);
    return JSON.parse(line) as JsonRpcResponse;
  }

  async function request(id: number, method: string, params: Record<string, unknown> = {}) {
    child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    return readResponse(method);
  }

  async function raw(frame: string): Promise<JsonRpcResponse> {
    child.stdin.write(`${frame}\n`);
    return readResponse(frame);
  }

  function notify(method: string, params: Record<string, unknown> = {}): void {
    child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method, params })}\n`);
  }

  async function close(): Promise<{ exitCode: number; stderr: string }> {
    child.stdin.end();
    const [exitCode, stderr] = await Promise.all([
      child.exited,
      new Response(child.stderr).text(),
    ]);
    return { exitCode, stderr };
  }

  return { close, notify, raw, request };
}

async function initialize(mcp: ReturnType<typeof startMcp>) {
  const response = await mcp.request(1, "initialize", {
    protocolVersion: "2025-06-18",
    capabilities: {},
    clientInfo: { name: "maestro-tests", version: "1" },
  });
  mcp.notify("notifications/initialized");
  return response;
}

test("42 MCP initialize carries the session brief as instructions and lists no tools (d891)", async () => {
  await withFixture(async (fixture) => {
    const mcp = startMcp(fixture);

    const initialized = await initialize(mcp);
    const listed = await mcp.request(2, "tools/list");
    const called = await mcp.request(3, "tools/call", {
      name: "maestro_run",
      arguments: { line: "work list" },
    });
    const closed = await mcp.close();
    const brief = await runCli(fixture, ["brief", "--session"], {
      MAESTRO_SESSION_ID: "test-session",
      MAESTRO_SESSION_PID: String(process.pid),
    });

    expect(initialized.error).toBeUndefined();
    expect(initialized.result?.serverInfo).toEqual({ name: "maestro", version });
    expect(initialized.result?.instructions).toContain("held work:");
    expect(initialized.result?.instructions).toContain("enabled policies:");
    expect(initialized.result?.instructions).toContain("recipes:");
    expect(initialized.result?.instructions).toBe(brief.stdout.trimEnd());
    expect(listed.result).toEqual({ tools: [] });
    expect(called.result).toBeUndefined();
    expect(called.error?.code).toBe(-32601);
    expect(closed).toEqual({ exitCode: 0, stderr: "" });
  });
});

test("305 MCP rejects a null frame and continues serving the same process", async () => {
  await withFixture(async (fixture) => {
    const mcp = startMcp(fixture);

    const invalid = await mcp.raw("null");
    const listed = await mcp.request(1, "tools/list");
    const closed = await mcp.close();

    expect(invalid).toEqual({
      jsonrpc: "2.0",
      id: null,
      error: { code: -32600, message: "invalid JSON-RPC request" },
    });
    expect(listed.result).toEqual({ tools: [] });
    expect(closed).toEqual({ exitCode: 0, stderr: "" });
  });
});
