/**
 * E2E — MCP stdio flow against ./dist/maestro mcp serve.
 *
 * Spawns the compiled binary as a long-lived stdio MCP server in a fresh
 * temp project and exercises each tool through real JSON-RPC messages.
 * This is the load-bearing handler-level coverage: the unit tests in
 * tests/unit/features/mcp/ only hit the helper layer.
 */
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { mkdir, mkdtemp, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  DIST_CLI,
  buildCompiledCli,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";
import { runCommand } from "../helpers/command-runner.js";

interface JsonRpcResponse {
  readonly jsonrpc: "2.0";
  readonly id: number;
  readonly result?: unknown;
  readonly error?: { code: number; message: string };
}

interface ToolPayload {
  readonly content: { type: "text"; text: string }[];
  readonly structuredContent?: Record<string, unknown>;
  readonly isError?: boolean;
}

class McpStdioClient {
  private buf = "";
  private pending = new Map<number, (msg: JsonRpcResponse) => void>();
  private nextId = 1;

  constructor(private readonly proc: ChildProcessWithoutNullStreams) {
    proc.stdout.on("data", (chunk) => this.onData(chunk.toString("utf8")));
  }

  private onData(text: string): void {
    this.buf += text;
    let idx;
    while ((idx = this.buf.indexOf("\n")) >= 0) {
      const line = this.buf.slice(0, idx).trim();
      this.buf = this.buf.slice(idx + 1);
      if (!line) continue;
      try {
        const msg = JSON.parse(line) as JsonRpcResponse;
        if (typeof msg.id === "number" && this.pending.has(msg.id)) {
          const fn = this.pending.get(msg.id)!;
          this.pending.delete(msg.id);
          fn(msg);
        }
      } catch {
        // ignore notifications and logs
      }
    }
  }

  async rpc(method: string, params: unknown = {}): Promise<JsonRpcResponse> {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        if (this.pending.has(id)) {
          this.pending.delete(id);
          reject(new Error(`RPC ${method} timed out (id=${id})`));
        }
      }, 10_000);
      timer.unref?.();
      this.pending.set(id, (msg) => {
        clearTimeout(timer);
        resolve(msg);
      });
      const payload = JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n";
      this.proc.stdin.write(payload);
    });
  }

  notify(method: string, params: unknown = {}): void {
    this.proc.stdin.write(JSON.stringify({ jsonrpc: "2.0", method, params }) + "\n");
  }

  async call(name: string, args: Record<string, unknown> = {}): Promise<{
    payload: ToolPayload;
    body: Record<string, unknown>;
  }> {
    const resp = await this.rpc("tools/call", { name, arguments: args });
    if (resp.error) {
      throw new Error(`tools/call(${name}) error: ${resp.error.message}`);
    }
    const payload = resp.result as ToolPayload;
    const text = payload.content?.[0]?.text ?? "{}";
    return { payload, body: JSON.parse(text) };
  }

  close(): void {
    this.proc.kill();
  }
}

let tmpDir: string;
let server: ChildProcessWithoutNullStreams | undefined;
let client: McpStdioClient | undefined;
let stderrBuf = "";

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-mcp-e2e-"));
  await initGitRepo(tmpDir);
  // maestro_policy_check / maestro_verdict_request resolve HEAD via git, so we need at
  // least one commit. Create an empty initial commit for that.
  await runCommand(["git", "config", "user.email", "e2e@maestro.test"], tmpDir);
  await runCommand(["git", "config", "user.name", "E2E"], tmpDir);
  await runCommand(["git", "commit", "--allow-empty", "-m", "init"], tmpDir);
  const initRes = await runCompiled(["init"], tmpDir);
  expect(initRes.exitCode).toBe(0);

  server = spawn(DIST_CLI, ["mcp", "serve"], {
    cwd: tmpDir,
    // HOME override isolates the handoff store (which roots at homedir())
    // so seeded handoff packets stay in tmpDir/.maestro/handoff/.
    env: { ...process.env, MAESTRO_PROJECT_ROOT: tmpDir, HOME: tmpDir },
    stdio: ["pipe", "pipe", "pipe"],
  });
  stderrBuf = "";
  server.stderr.on("data", (c) => {
    stderrBuf += c.toString("utf8");
  });

  client = new McpStdioClient(server);
  const init = await client.rpc("initialize", {
    protocolVersion: "2025-03-26",
    capabilities: {},
    clientInfo: { name: "e2e", version: "0" },
  });
  expect(init.error).toBeUndefined();
  client.notify("notifications/initialized");
});

afterEach(async () => {
  client?.close();
  client = undefined;
  if (server && !server.killed) {
    server.kill();
  }
  server = undefined;
  await rm(tmpDir, { recursive: true, force: true });
});

afterAll(() => {
  // No-op: each test fully cleans up. Kept for parity with sibling e2e files.
});

describe("MCP stdio flow", () => {
  it("creates, gets, and lists a task", async () => {
    const c = client!;
    const created = await c.call("maestro_task_create", { title: "e2e task" });
    const taskId = (created.body as { task: { id: string } }).task.id;
    expect(taskId).toMatch(/^tsk-[0-9a-f]{6}$/);

    const got = await c.call("maestro_task_get", { id: taskId });
    expect((got.body as { task: { id: string; title: string } }).task.title).toBe("e2e task");

    const list = await c.call("maestro_task_list", {});
    const items = (list.body as { items: { id: string }[] }).items;
    expect(items.some((t) => t.id === taskId)).toBe(true);
  });

  it("returns a TASK_NOT_FOUND error for an unknown id", async () => {
    const c = client!;
    const r = await c.call("maestro_task_get", { id: "tsk-deadbe" });
    expect(r.payload.isError).toBe(true);
    expect((r.body as { code: string }).code).toBe("TASK_NOT_FOUND");
  });

  it("claims a task and completes it", async () => {
    const c = client!;
    const created = await c.call("maestro_task_create", { title: "claim-me" });
    const taskId = (created.body as { task: { id: string } }).task.id;

    const claimed = await c.call("maestro_task_claim", { id: taskId });
    const claimedTask = (claimed.body as { task: { assignee?: string; claimedAt?: string } }).task;
    expect(typeof claimedTask.assignee).toBe("string");
    expect(typeof claimedTask.claimedAt).toBe("string");

    const completed = await c.call("maestro_task_complete", { id: taskId, summary: "done" });
    expect((completed.body as { task: { status: string } }).task.status).toBe("completed");
  });

  it("records and lists evidence rows", async () => {
    const c = client!;
    const created = await c.call("maestro_task_create", { title: "evidence-flow" });
    const taskId = (created.body as { task: { id: string } }).task.id;

    const cmdEv = await c.call("maestro_evidence_record", {
      taskId,
      command: "echo ok",
      exitCode: 0,
      witnessLevel: "agent-claimed-locally",
    });
    expect((cmdEv.body as { evidence: { id: string } }).evidence.id).toMatch(/^evd-/);

    const noteEv = await c.call("maestro_evidence_record", { taskId, note: "manual" });
    expect((noteEv.body as { evidence: { id: string } }).evidence.id).toMatch(/^evd-/);

    const list = await c.call("maestro_evidence_list", { taskId });
    const items = (list.body as { items: { id: string }[] }).items;
    expect(items.length).toBeGreaterThanOrEqual(2);
  });

  it("maintains bidirectional block edges and detects self-block", async () => {
    const c = client!;
    const a = (
      (await c.call("maestro_task_create", { title: "a" })).body as { task: { id: string } }
    ).task.id;
    const b = (
      (await c.call("maestro_task_create", { title: "b" })).body as { task: { id: string } }
    ).task.id;

    await c.call("maestro_task_block", { id: a, blockedTaskIds: [b] });
    const aAfter = (
      (await c.call("maestro_task_get", { id: a })).body as {
        task: { blocks: string[] };
      }
    ).task.blocks;
    const bAfter = (
      (await c.call("maestro_task_get", { id: b })).body as {
        task: { blockedBy: string[] };
      }
    ).task.blockedBy;
    expect(aAfter).toContain(b);
    expect(bAfter).toContain(a);

    const selfBlock = await c.call("maestro_task_block", { id: a, blockedTaskIds: [a] });
    expect(selfBlock.payload.isError).toBe(true);
  });

  it("shows, amends, and re-shows a contract", async () => {
    const c = client!;
    const created = await c.call("maestro_task_create", { title: "contract-flow" });
    const taskId = (created.body as { task: { id: string } }).task.id;

    // Seed a v1 contract directly. We need the real on-disk shape that
    // validateContract accepts: see src/features/task/domain/contract/.
    const contractDir = join(tmpDir, ".maestro", "contracts", taskId);
    await mkdir(contractDir, { recursive: true });
    const contract = {
      schemaVersion: 2,
      id: "c-abc123",
      taskId,
      repoRoot: tmpDir,
      status: "locked",
      createdAt: new Date().toISOString(),
      intent: "e2e contract path",
      scope: { filesExpected: ["src/foo.ts"], filesForbidden: [] },
      doneWhen: [],
      amendments: [],
      createdBy: "session:e2e",
      configSnapshot: {
        strict: false,
        overlapPolicy: "fail",
        rebaseFallback: "best-effort",
        staleReclaimContractPolicy: "inherit",
      },
      riskClass: "low",
    };
    await writeFile(join(contractDir, "v1.json"), JSON.stringify(contract));

    const show1 = await c.call("maestro_contract_show", { taskId });
    expect(
      (
        show1.body as {
          contract: { taskId: string; scope: { filesExpected: string[] } };
        }
      ).contract.scope.filesExpected,
    ).toEqual(["src/foo.ts"]);

    const amended = await c.call("maestro_contract_amend", {
      taskId,
      addPaths: ["src/bar.ts"],
      reason: "scope creep",
    });
    expect((amended.body as { newVersion: number }).newVersion).toBe(2);

    const show2 = await c.call("maestro_contract_show", { taskId });
    const expectedAfter = (
      show2.body as { contract: { scope: { filesExpected: string[] } } }
    ).contract.scope.filesExpected;
    expect(expectedAfter).toContain("src/foo.ts");
    expect(expectedAfter).toContain("src/bar.ts");

    const noChange = await c.call("maestro_contract_amend", {
      taskId,
      addPaths: ["src/bar.ts"],
      reason: "noop",
    });
    expect(noChange.payload.isError).toBe(true);
    expect((noChange.body as { code: string }).code).toBe("NO_SCOPE_CHANGES");
  });

  it("requests a verdict and shows it back", async () => {
    const c = client!;
    const created = await c.call("maestro_task_create", { title: "verdict-flow" });
    const taskId = (created.body as { task: { id: string } }).task.id;

    const contractDir = join(tmpDir, ".maestro", "contracts", taskId);
    await mkdir(contractDir, { recursive: true });
    await writeFile(
      join(contractDir, "v1.json"),
      JSON.stringify({
        schemaVersion: 2,
        id: "c-def456",
        taskId,
        repoRoot: tmpDir,
        status: "locked",
        createdAt: new Date().toISOString(),
        intent: "verdict",
        scope: { filesExpected: ["src/x.ts"], filesForbidden: [] },
        doneWhen: [],
        amendments: [],
        createdBy: "session:e2e",
        configSnapshot: {
          strict: false,
          overlapPolicy: "fail",
          rebaseFallback: "best-effort",
          staleReclaimContractPolicy: "inherit",
        },
        riskClass: "low",
      }),
    );

    const before = await c.call("maestro_verdict_show", { taskId });
    expect(before.payload.isError).toBe(true);
    expect((before.body as { code: string }).code).toBe("VERDICT_NOT_FOUND");

    const requested = await c.call("maestro_verdict_request", { taskId });
    const decision = (requested.body as { verdict: { decision: string } }).verdict.decision;
    expect(["PASS", "FAIL", "HUMAN", "BLOCK"]).toContain(decision);

    const after = await c.call("maestro_verdict_show", { taskId });
    expect((after.body as { verdict: { decision: string } }).verdict.decision).toBe(decision);
  });

  it("computes policy effective risk class for a task with a contract", async () => {
    const c = client!;
    const created = await c.call("maestro_task_create", { title: "policy-flow" });
    const taskId = (created.body as { task: { id: string } }).task.id;

    const contractDir = join(tmpDir, ".maestro", "contracts", taskId);
    await mkdir(contractDir, { recursive: true });
    await writeFile(
      join(contractDir, "v1.json"),
      JSON.stringify({
        schemaVersion: 2,
        id: "c-aaa111",
        taskId,
        repoRoot: tmpDir,
        status: "locked",
        createdAt: new Date().toISOString(),
        intent: "policy",
        scope: { filesExpected: ["src/x.ts"], filesForbidden: [] },
        doneWhen: [],
        amendments: [],
        createdBy: "session:e2e",
        configSnapshot: {
          strict: false,
          overlapPolicy: "fail",
          rebaseFallback: "best-effort",
          staleReclaimContractPolicy: "inherit",
        },
        riskClass: "low",
      }),
    );

    const r = await c.call("maestro_policy_check", { taskId });
    if (r.payload.isError) {
      throw new Error(`maestro_policy_check returned error: ${JSON.stringify(r.body)}`);
    }
    const body = r.body as {
      effectiveRiskClass: string;
      derivedRiskClass: string;
      contractRiskClass: string;
    };
    expect(["low", "medium", "high", "critical"]).toContain(body.effectiveRiskClass);
    expect(["low", "medium", "high", "critical"]).toContain(body.derivedRiskClass);
    expect(body.contractRiskClass).toBe("low");
  });

  it("rejects an empty title at the schema layer", async () => {
    const c = client!;
    const r = await c.rpc("tools/call", {
      name: "maestro_task_create",
      arguments: { title: "" },
    });
    // The MCP SDK may surface a zod validation failure as a JSON-RPC
    // error or as a tool result with isError=true; accept either path.
    const tooled = r.result as ToolPayload | undefined;
    const rejected =
      r.error !== undefined ||
      (tooled !== undefined && tooled.isError === true);
    expect(rejected).toBe(true);
  });

  it("returns a stable error code when no contract exists for maestro_contract_show", async () => {
    const c = client!;
    const created = await c.call("maestro_task_create", { title: "no-contract" });
    const taskId = (created.body as { task: { id: string } }).task.id;
    const r = await c.call("maestro_contract_show", { taskId });
    expect(r.payload.isError).toBe(true);
    expect((r.body as { code: string }).code).toBe("CONTRACT_NOT_FOUND");
  });

  it("lists, shows, and picks up a standalone handoff packet", async () => {
    const c = client!;
    // The MCP server resolves projectRoot via realpath; macOS tmpdir is a
    // symlink to /private/var/..., so the seeded record's refs.projectRoot
    // must use the realpath'd path for `isHandoffInProject` to match.
    const realTmpDir = await realpath(tmpDir);
    const handoffId = "swift-otter-1";
    const handoffDir = join(tmpDir, ".maestro", "handoff", handoffId);
    await mkdir(handoffDir, { recursive: true });
    const record = {
      id: handoffId,
      createdAt: new Date().toISOString(),
      task: "ship the thing",
      name: "ship the thing",
      agent: "codex",
      model: "gpt-5.4",
      status: "launched",
      wait: false,
      sourceDir: realTmpDir,
      targetDir: realTmpDir,
      promptPath: join(".maestro", "handoff", handoffId, "prompt.md"),
      outputPath: join(".maestro", "handoff", handoffId, "output.log"),
      command: ["codex"],
      refs: { projectRoot: realTmpDir },
    };
    await writeFile(join(handoffDir, "handoff.json"), JSON.stringify(record));
    await writeFile(join(handoffDir, "prompt.md"), "## task\nship the thing\n");
    await writeFile(join(handoffDir, "output.log"), "");

    const list = await c.call("maestro_handoff_list", { openOnly: true });
    const items = (list.body as { items: { id: string }[] }).items;
    expect(items.some((r) => r.id === handoffId)).toBe(true);

    // displayState=open is the explicit form of openOnly=true; same coverage.
    const listByState = await c.call("maestro_handoff_list", { displayState: "open" });
    const stateItems = (listByState.body as { items: { id: string }[] }).items;
    expect(stateItems.some((r) => r.id === handoffId)).toBe(true);

    // agent filter narrows correctly.
    const listByAgent = await c.call("maestro_handoff_list", { agent: "codex" });
    expect((listByAgent.body as { items: { id: string }[] }).items.some((r) => r.id === handoffId)).toBe(true);
    const listByOtherAgent = await c.call("maestro_handoff_list", { agent: "claude" });
    expect((listByOtherAgent.body as { items: { id: string }[] }).items.some((r) => r.id === handoffId)).toBe(false);

    // openOnly + displayState combination is rejected by the handler with a
    // stable error code (the schema layer accepts both so SDK can serialize
    // properties for tools/list).
    const conflict = await c.call("maestro_handoff_list", {
      openOnly: true,
      displayState: "consumed",
    });
    expect(conflict.payload.isError).toBe(true);
    expect((conflict.body as { code: string }).code).toBe("INVALID_FILTER_COMBINATION");

    const shown = await c.call("maestro_handoff_show", { id: handoffId });
    expect((shown.body as { record: { id: string; agent: string } }).record.id).toBe(handoffId);
    expect((shown.body as { record: { agent: string } }).record.agent).toBe("codex");

    const missing = await c.call("maestro_handoff_show", { id: "missing-pkt-9" });
    expect(missing.payload.isError).toBe(true);
    expect((missing.body as { code: string }).code).toBe("HANDOFF_NOT_FOUND");

    // open_for_task on a packet with no taskId returns an empty list.
    const noTaskOpen = await c.call("maestro_handoff_open_for_task", {
      taskId: "tsk-deadbeef",
    });
    expect((noTaskOpen.body as { handoffIds: string[] }).handoffIds).toEqual([]);

    const picked = await c.call("maestro_handoff_pickup", {
      id: handoffId,
      actorAgent: "codex",
      actorSessionId: "e2e-session",
      standalone: true,
    });
    const pickedRecord = (picked.body as { record: { status: string; consumedAt?: string } }).record;
    expect(pickedRecord.status).toBe("consumed");
    expect(typeof pickedRecord.consumedAt).toBe("string");

    const replay = await c.call("maestro_handoff_pickup", {
      id: handoffId,
      actorAgent: "codex",
      actorSessionId: "e2e-session",
      standalone: true,
    });
    expect(replay.payload.isError).toBe(true);
    expect((replay.body as { code: string }).code).toBe("ALREADY_CONSUMED");
  });

  it("returns CROSS_PROJECT_PICKUP when a task-linked packet is picked up from a foreign project", async () => {
    const c = client!;
    const handoffId = "wise-fox-2";
    const realTmpDir = await realpath(tmpDir);
    const handoffDir = join(tmpDir, ".maestro", "handoff", handoffId);
    await mkdir(handoffDir, { recursive: true });
    // sourceDir / refs.projectRoot point at a *different* project than the
    // running MCP server's project. The pickup use-case must refuse and
    // surface a CROSS_PROJECT_PICKUP error.
    const foreignProject = join(realTmpDir, "foreign-project-not-real");
    const record = {
      id: handoffId,
      createdAt: new Date().toISOString(),
      task: "from another tree",
      name: "from another tree",
      agent: "claude",
      model: "opus",
      status: "launched",
      wait: false,
      sourceDir: foreignProject,
      targetDir: foreignProject,
      promptPath: join(".maestro", "handoff", handoffId, "prompt.md"),
      outputPath: join(".maestro", "handoff", handoffId, "output.log"),
      command: ["claude"],
      refs: { projectRoot: foreignProject, taskId: "tsk-foreign1" },
    };
    await writeFile(join(handoffDir, "handoff.json"), JSON.stringify(record));
    await writeFile(join(handoffDir, "prompt.md"), "## task\n");
    await writeFile(join(handoffDir, "output.log"), "");

    // The packet is project-scoped so `show` from this project returns
    // HANDOFF_NOT_FOUND.
    const blocked = await c.call("maestro_handoff_show", { id: handoffId });
    expect(blocked.payload.isError).toBe(true);
    expect((blocked.body as { code: string }).code).toBe("HANDOFF_NOT_FOUND");

    // Pickup goes through `handoffStore.get()` directly (not the project
    // scope filter), so the cross-project guard in pickup-handoff.usecase.ts
    // fires and the handler must surface CROSS_PROJECT_PICKUP with hints
    // preserved from the underlying MaestroError.
    const cross = await c.call("maestro_handoff_pickup", {
      id: handoffId,
      actorAgent: "claude",
      actorSessionId: "e2e-cross-session",
    });
    expect(cross.payload.isError).toBe(true);
    const crossBody = cross.body as {
      code: string;
      message: string;
      hints: string[];
    };
    expect(crossBody.code).toBe("CROSS_PROJECT_PICKUP");
    expect(crossBody.message).toContain("belongs to project");
    expect(crossBody.hints.length).toBeGreaterThan(0);
    expect(crossBody.hints.some((h) => h.includes("source project"))).toBe(true);
  });

  it("rejects unknown fields at the schema boundary (strict mode)", async () => {
    const c = client!;
    // maestro_task_create with a typo'd 'missionID' (correct field would be
    // 'missionId', but maestro_task_create no longer accepts it at all).
    const r = await c.rpc("tools/call", {
      name: "maestro_task_create",
      arguments: { title: "strict mode probe", missionID: "msn-abc123" },
    });
    const tooled = r.result as ToolPayload | undefined;
    const rejected =
      r.error !== undefined ||
      (tooled !== undefined && tooled.isError === true);
    expect(rejected).toBe(true);
  });

  it("returns lean INVALID_ARG shape for missing required args", async () => {
    const c = client!;
    const r = await c.rpc("tools/call", {
      name: "maestro_evidence_list",
      arguments: {},
    });
    const wire = JSON.stringify(r);
    // Doctrine target: full JSON-RPC line for a single missing arg < 200 B.
    expect(wire.length).toBeLessThan(200);
    const payload = r.result as ToolPayload;
    expect(payload.isError).toBe(true);
    expect(payload.structuredContent).toBeUndefined();
    const body = JSON.parse(payload.content[0]!.text) as Record<string, unknown>;
    expect(body.code).toBe("INVALID_ARG");
    expect(body.arg).toBe("taskId");
    expect(typeof body.message).toBe("string");
  });
});
