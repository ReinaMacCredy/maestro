import { expect, test } from "bun:test";
import { join } from "node:path";
import {
  idFrom,
  runCli,
  withFixture,
  writeConfig,
  writePlugin,
  type Fixture,
} from "./helpers.ts";

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

test("42 MCP initialize carries the dynamic brief and lists exactly two meta-tools", async () => {
  await withFixture(async (fixture) => {
    const mcp = startMcp(fixture);

    const initialized = await initialize(mcp);
    const listed = await mcp.request(2, "tools/list");
    const closed = await mcp.close();
    const tools = (listed.result?.tools ?? []) as Array<{ name: string }>;

    expect(initialized.error).toBeUndefined();
    expect(initialized.result?.serverInfo).toEqual({ name: "maestro", version: "0.113.1" });
    expect(initialized.result?.instructions).toContain("held work:");
    expect(initialized.result?.instructions).toContain("enabled policies:");
    expect(initialized.result?.instructions).not.toContain("pending message");
    expect(initialized.result?.instructions).toContain("recipes:");
    expect(tools.map((tool) => tool.name)).toEqual(["maestro_find", "maestro_run"]);
    expect(closed).toEqual({ exitCode: 0, stderr: "" });
  });
});

test("43 maestro_find reads the live registry and recipes while maestro_run uses normal dispatch", async () => {
  await withFixture(async (fixture) => {
    await writePlugin(
      fixture,
      "repo",
      "moon-tools",
      `
export default {
  name: "moon-tools",
  apply(ctx) {
    ctx.effect(() => ctx.cli.register(
      "moon launch",
      () => ({ data: { launched: true }, text: "launched" }),
      {
        description: "Launch moon work through a plugin verb.",
        flags: { "--target": { description: "Choose the launch target.", value: true } },
      },
    ));
  },
};
`,
    );
    await writePlugin(
      fixture,
      "repo",
      "moon-disabled",
      `export default {
  name: "moon-disabled",
  apply(ctx) {
    ctx.effect(() => ctx.cli.register("moon secret", () => "hidden"));
  },
};
`,
    );
    await writeConfig(fixture, [{ name: "moon-disabled", disabled: true }]);
    const mcp = startMcp(fixture);
    await initialize(mcp);

    const pluginFind = await mcp.request(2, "tools/call", {
      name: "maestro_find",
      arguments: { query: "moon" },
    });
    const recipeFind = await mcp.request(3, "tools/call", {
      name: "maestro_find",
      arguments: { query: "work" },
    });
    const ran = await mcp.request(4, "tools/call", {
      name: "maestro_run",
      arguments: { line: 'work add "MCP created item" --kind idea' },
    });
    const strict = await mcp.request(5, "tools/call", {
      name: "maestro_run",
      arguments: { line: "work list discarded-positional" },
    });
    const closed = await mcp.close();
    const pluginResult = pluginFind.result?.structuredContent as {
      recipes: Array<{ name: string }>;
      verbs: Array<{
        description: string;
        flags: Array<{
          description: string;
          multiple: boolean;
          name: string;
          value: boolean;
        }>;
        name: string;
      }>;
    };
    const recipeResult = recipeFind.result?.structuredContent as {
      recipes: Array<{ name: string }>;
    };
    const runEnvelope = ran.result?.structuredContent as {
      data: { work: { title: string } };
      ok: boolean;
    };
    const listed = await runCli(fixture, ["work", "list"]);

    expect(pluginResult.verbs).toContainEqual({
      name: "moon launch",
      description: "Launch moon work through a plugin verb.",
      flags: [
        {
          name: "--target",
          description: "Choose the launch target.",
          multiple: false,
          value: true,
        },
      ],
    });
    expect(JSON.stringify(pluginResult)).not.toContain("moon secret");
    expect(recipeResult.recipes.map((recipe) => recipe.name)).toContain("work");
    expect(runEnvelope.ok).toBe(true);
    expect(runEnvelope.data.work.title).toBe("MCP created item");
    expect(listed.stdout).toContain("MCP created item");
    expect(strict.result?.isError).toBe(true);
    expect(JSON.stringify(strict.result)).toContain("UNKNOWN_ARGUMENT");
    expect(closed).toEqual({ exitCode: 0, stderr: "" });
  });
});

test("44 policy gates reached through maestro_run return tool errors with unblocking commands", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(await runCli(fixture, ["work", "add", "MCP gated", "--kind", "idea"]));
    expect(await runCli(fixture, ["work", "start", work])).toHaveProperty("exitCode", 0);
    const mcp = startMcp(fixture);
    await initialize(mcp);

    const blocked = await mcp.request(2, "tools/call", {
      name: "maestro_run",
      arguments: { line: `work done ${work} --claim "done"` },
    });
    const closed = await mcp.close();

    expect(blocked.result?.isError).toBe(true);
    expect(JSON.stringify(blocked.result)).toContain("GATE_BLOCKED");
    expect(JSON.stringify(blocked.result)).toContain("policy-proof");
    expect(JSON.stringify(blocked.result)).toContain(`maestro work done ${work}`);
    expect(JSON.stringify(blocked.result)).toContain("--proof");
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
    expect((listed.result?.tools as Array<{ name: string }>).map((tool) => tool.name)).toEqual([
      "maestro_find",
      "maestro_run",
    ]);
    expect(closed).toEqual({ exitCode: 0, stderr: "" });
  });
});
