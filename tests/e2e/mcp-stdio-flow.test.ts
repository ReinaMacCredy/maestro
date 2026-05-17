/**
 * E2E — MCP stdio flow against ./dist/maestro mcp serve.
 *
 * Spawns the compiled binary as a long-lived stdio MCP server in a fresh
 * temp project and exercises each tool through real JSON-RPC messages.
 * This is the load-bearing handler-level coverage: the unit tests in
 * tests/unit/features/mcp/ only hit the helper layer.
 *
 * v2 update (D-task-MCP): uses v2 verbs (task_from_spec, task_ship,
 * task_block with reason). Seeds spec files and task JSONL for flows that
 * need pre-existing state.
 */
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
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

/** Write a minimal v2 product-spec markdown file and return the absolute path. */
async function writeSpec(dir: string, slug: string, title: string): Promise<string> {
  const specDir = join(dir, ".maestro", "specs");
  await mkdir(specDir, { recursive: true });
  const path = join(specDir, `${slug}.md`);
  const content = [
    "---",
    `slug: ${slug}`,
    "mode: light",
    "risk_class: low",
    "work_type: maintenance",
    "acceptance_criteria:",
    `  - ${title} is complete`,
    "---",
    "",
    `# ${title}`,
    "",
    "E2E test spec.",
  ].join("\n");
  await writeFile(path, content);
  return path;
}

/** Seed a v2 task directly into the JSONL store (bypasses state machine). */
async function seedV2Task(dir: string, task: Record<string, unknown>): Promise<void> {
  const tasksDir = join(dir, ".maestro", "tasks");
  await mkdir(tasksDir, { recursive: true });
  const file = join(tasksDir, "tasks.jsonl");
  const line = JSON.stringify(task) + "\n";
  await writeFile(file, line, { flag: "a" });
}

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

  // v2 directories created by maestro init (Phase 3 will surface this via setup).
  await mkdir(join(tmpDir, ".maestro/tasks"), { recursive: true });
  await mkdir(join(tmpDir, ".maestro/plans"), { recursive: true });
  await mkdir(join(tmpDir, ".maestro/evidence"), { recursive: true });
  await mkdir(join(tmpDir, ".maestro/runs"), { recursive: true });
  await mkdir(join(tmpDir, "docs/principles"), { recursive: true });

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
  it("creates a task from spec, gets it, and lists it", async () => {
    const c = client!;
    const specPath = await writeSpec(tmpDir, "e2e-task", "E2E Task");

    const created = await c.call("maestro_task_from_spec", { spec_path: specPath });
    const taskId = (created.body as { task: { id: string } }).task.id;
    expect(taskId).toMatch(/^tsk-[a-z0-9]+-[a-z0-9]+$/);

    const got = await c.call("maestro_task_get", { id: taskId });
    expect((got.body as { task: { id: string; title: string } }).task.title).toBe("E2E Task");

    const list = await c.call("maestro_task_list", {});
    const items = (list.body as { items: { id: string }[] }).items;
    expect(items.some((t) => t.id === taskId)).toBe(true);
  });

  it("returns a TASK_NOT_FOUND error for an unknown id", async () => {
    const c = client!;
    // v2 task ID format — not present in store.
    const r = await c.call("maestro_task_get", { id: "tsk-aaaa00-bbbb00" });
    expect(r.payload.isError).toBe(true);
    expect((r.body as { code: string }).code).toBe("TASK_NOT_FOUND");
  });

  it("creates a task from spec and claims it", async () => {
    const c = client!;
    const specPath = await writeSpec(tmpDir, "claim-me", "Claim Me");

    const created = await c.call("maestro_task_from_spec", { spec_path: specPath });
    const taskId = (created.body as { task: { id: string } }).task.id;

    const claimed = await c.call("maestro_task_claim", { id: taskId });
    const claimedTask = (claimed.body as { task: { assignee?: string; claimed_at?: string; state: string } }).task;
    expect(claimedTask.state).toBe("claimed");
    expect(typeof claimedTask.assignee).toBe("string");
  });

  it("blocks a claimed task with a reason", async () => {
    const c = client!;
    const specPath = await writeSpec(tmpDir, "block-me", "Block Me");

    const created = await c.call("maestro_task_from_spec", { spec_path: specPath });
    const taskId = (created.body as { task: { id: string } }).task.id;

    await c.call("maestro_task_claim", { id: taskId });

    const blocked = await c.call("maestro_task_block", { id: taskId, reason: "waiting on infra" });
    const blockedTask = (blocked.body as { task: { state: string; block_reason?: string } }).task;
    expect(blockedTask.state).toBe("blocked");
    expect(blockedTask.block_reason).toBe("waiting on infra");
  });

  it("ships a task seeded in ready state", async () => {
    const c = client!;
    // Seed a task directly in 'ready' state (state machine: ready -> shipped).
    const now = new Date().toISOString();
    const taskId = "tsk-" + Date.now().toString(36) + "-" + Math.random().toString(36).slice(2, 8);
    await seedV2Task(tmpDir, {
      id: taskId,
      slug: "ship-me/ready-task",
      title: "Ship Me",
      state: "ready",
      blocked_by: [],
      created_at: now,
      updated_at: now,
    });

    const shipped = await c.call("maestro_task_ship", {
      id: taskId,
      pr_url: "https://github.com/owner/repo/pull/42",
    });
    const shippedTask = (shipped.body as { task: { state: string; pr_url?: string } }).task;
    expect(shippedTask.state).toBe("shipped");
    expect(shippedTask.pr_url).toBe("https://github.com/owner/repo/pull/42");
  });

  it("records and lists evidence rows", async () => {
    const c = client!;
    const specPath = await writeSpec(tmpDir, "evidence-flow", "Evidence Flow");

    const created = await c.call("maestro_task_from_spec", { spec_path: specPath });
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

  it("task_block requires a reason (rejects missing reason)", async () => {
    const c = client!;
    const r = await c.rpc("tools/call", {
      name: "maestro_task_block",
      arguments: { id: "tsk-aaaa00-bbbb00" },
    });
    // Schema rejects missing reason at the zod layer.
    const tooled = r.result as ToolPayload | undefined;
    const rejected =
      r.error !== undefined ||
      (tooled !== undefined && tooled.isError === true);
    expect(rejected).toBe(true);
  });

  it("shows, amends, and re-shows a contract", async () => {
    const c = client!;
    const specPath = await writeSpec(tmpDir, "contract-flow", "Contract Flow");
    const created = await c.call("maestro_task_from_spec", { spec_path: specPath });
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
    const specPath = await writeSpec(tmpDir, "verdict-flow", "Verdict Flow");
    const created = await c.call("maestro_task_from_spec", { spec_path: specPath });
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
    const specPath = await writeSpec(tmpDir, "policy-flow", "Policy Flow");
    const created = await c.call("maestro_task_from_spec", { spec_path: specPath });
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

  it("rejects an empty spec_path at the schema layer", async () => {
    const c = client!;
    const r = await c.rpc("tools/call", {
      name: "maestro_task_from_spec",
      arguments: { spec_path: "" },
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
    const specPath = await writeSpec(tmpDir, "no-contract", "No Contract");
    const created = await c.call("maestro_task_from_spec", { spec_path: specPath });
    const taskId = (created.body as { task: { id: string } }).task.id;
    const r = await c.call("maestro_contract_show", { taskId });
    expect(r.payload.isError).toBe(true);
    expect((r.body as { code: string }).code).toBe("CONTRACT_NOT_FOUND");
  });

  it("rejects unknown fields at the schema boundary (strict mode)", async () => {
    const c = client!;
    // maestro_task_from_spec with an unknown extra field.
    const r = await c.rpc("tools/call", {
      name: "maestro_task_from_spec",
      arguments: { spec_path: "docs/specs/foo.md", missionID: "msn-abc123" },
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

  it("maestro_setup_check returns a report with ok status", async () => {
    const c = client!;
    const r = await c.call("maestro_setup_check", {});
    if (r.payload.isError) {
      throw new Error(`maestro_setup_check failed: ${JSON.stringify(r.body)}`);
    }
    const body = r.body as { ok: boolean; entries: { path: string; status: string }[] };
    expect(typeof body.ok).toBe("boolean");
    expect(Array.isArray(body.entries)).toBe(true);
    // After bootstrap, all directories should be ok (not missing).
    const missing = body.entries.filter((e) => e.status === "missing");
    expect(missing).toEqual([]);
  });

  it("emits a handoff envelope and lists it", async () => {
    const c = client!;
    const specPath = await writeSpec(tmpDir, "handoff-emit", "Handoff Emit");
    const created = await c.call("maestro_task_from_spec", { spec_path: specPath });
    const taskId = (created.body as { task: { id: string } }).task.id;

    const emitted = await c.call("maestro_handoff_emit", {
      task_id: taskId,
      trigger_verb: "task:abandon",
      reason: "scope changed",
    });
    const envelope = (emitted.body as { envelope: { id: string; task_id: string; trigger_verb: string } }).envelope;
    expect(envelope.id).toMatch(/^hnd-[a-z0-9]+-[a-z0-9]+$/);
    expect(envelope.task_id).toBe(taskId);
    expect(envelope.trigger_verb).toBe("task:abandon");

    const list = await c.call("maestro_handoff_list", { task_id: taskId });
    const items = (list.body as { items: { id: string; picked_up: boolean }[] }).items;
    expect(items.some((i) => i.id === envelope.id)).toBe(true);
    expect(items.find((i) => i.id === envelope.id)?.picked_up).toBe(false);
  });

  it("rejects task:block emit without a reason at the schema layer", async () => {
    const c = client!;
    const r = await c.rpc("tools/call", {
      name: "maestro_handoff_emit",
      arguments: { task_id: "tsk-abc123", trigger_verb: "task:block" },
    });
    const tooled = r.result as ToolPayload | undefined;
    const body = JSON.parse(tooled?.content[0]?.text ?? "{}") as { code?: string; arg?: string };
    expect(tooled?.isError).toBe(true);
    expect(body.code).toBe("INVALID_ARG");
    expect(body.arg).toBe("reason");
  });

  it("shows a handoff envelope by id", async () => {
    const c = client!;
    const specPath = await writeSpec(tmpDir, "handoff-show", "Handoff Show");
    const created = await c.call("maestro_task_from_spec", { spec_path: specPath });
    const taskId = (created.body as { task: { id: string } }).task.id;

    const emitted = await c.call("maestro_handoff_emit", {
      task_id: taskId,
      trigger_verb: "task:verify",
    });
    const id = (emitted.body as { envelope: { id: string } }).envelope.id;

    const shown = await c.call("maestro_handoff_show", { id });
    const body = shown.body as { envelope: { id: string }; picked_up?: unknown };
    expect(body.envelope.id).toBe(id);
    expect(body.picked_up).toBeUndefined();
  });

  it("returns HANDOFF_NOT_FOUND for an unknown envelope id", async () => {
    const c = client!;
    const r = await c.call("maestro_handoff_show", { id: "hnd-zzzzz-zzzzzz" });
    expect(r.payload.isError).toBe(true);
    expect((r.body as { code: string }).code).toBe("HANDOFF_NOT_FOUND");
  });

  it("marks a handoff as picked up; second pickup returns ALREADY_PICKED_UP", async () => {
    const c = client!;
    const specPath = await writeSpec(tmpDir, "handoff-pickup", "Handoff Pickup");
    const created = await c.call("maestro_task_from_spec", { spec_path: specPath });
    const taskId = (created.body as { task: { id: string } }).task.id;

    const emitted = await c.call("maestro_handoff_emit", {
      task_id: taskId,
      trigger_verb: "task:claim",
    });
    const id = (emitted.body as { envelope: { id: string } }).envelope.id;

    const first = await c.call("maestro_handoff_pickup", {
      id,
      picked_up_by: "agent-pickup",
      note: "taking over",
    });
    const firstBody = first.body as {
      envelope: { id: string };
      pickup: { id: string; envelope_id: string; picked_up_by: string };
    };
    expect(firstBody.envelope.id).toBe(id);
    expect(firstBody.pickup.envelope_id).toBe(id);
    expect(firstBody.pickup.picked_up_by).toBe("agent-pickup");

    const second = await c.call("maestro_handoff_pickup", {
      id,
      picked_up_by: "agent-pickup-2",
    });
    expect(second.payload.isError).toBe(true);
    expect((second.body as { code: string }).code).toBe("HANDOFF_ALREADY_PICKED_UP");

    // After pickup, default list excludes it; include_picked_up surfaces it.
    const openList = await c.call("maestro_handoff_list", { task_id: taskId });
    const openItems = (openList.body as { items: { id: string }[] }).items;
    expect(openItems.some((i) => i.id === id)).toBe(false);

    const allList = await c.call("maestro_handoff_list", {
      task_id: taskId,
      include_picked_up: true,
    });
    const allItems = (allList.body as { items: { id: string; picked_up: boolean }[] }).items;
    const row = allItems.find((i) => i.id === id);
    expect(row).toBeDefined();
    expect(row?.picked_up).toBe(true);
  });

  it("maestro_task_list filters by state", async () => {
    const c = client!;
    const specPath = await writeSpec(tmpDir, "state-filter", "State Filter");
    await c.call("maestro_task_from_spec", { spec_path: specPath });

    // Filter by state=draft; the newly created task should appear.
    const draftList = await c.call("maestro_task_list", { state: "draft" });
    const drafts = (draftList.body as { items: { state: string }[] }).items;
    expect(drafts.every((t) => t.state === "draft")).toBe(true);
    expect(drafts.length).toBeGreaterThanOrEqual(1);

    // Filter by state=shipped; should be empty in a fresh project.
    const shippedList = await c.call("maestro_task_list", { state: "shipped" });
    const shipped = (shippedList.body as { items: unknown[] }).items;
    expect(shipped.length).toBe(0);
  });

  it("maestro_task_list composes plan_id and state filters", async () => {
    const c = client!;
    const specPath = await writeSpec(tmpDir, "compose-filter", "Compose Filter");
    const created = await c.call("maestro_task_from_spec", { spec_path: specPath });
    const taskId = (created.body as { task: { id: string; plan_id?: string } }).task.id;
    const planId = (created.body as { task: { plan_id?: string } }).task.plan_id;

    // Without plan_id the task should appear under state=draft.
    const draftAll = await c.call("maestro_task_list", { state: "draft" });
    const draftItems = (draftAll.body as { items: { id: string }[] }).items;
    expect(draftItems.find((t) => t.id === taskId)).toBeDefined();

    if (planId !== undefined) {
      // With both filters, must still find the draft task in this plan.
      const both = await c.call("maestro_task_list", { plan_id: planId, state: "draft" });
      const bothItems = (both.body as { items: { id: string; state: string }[] }).items;
      expect(bothItems.every((t) => t.state === "draft")).toBe(true);
      expect(bothItems.find((t) => t.id === taskId)).toBeDefined();

      // And no draft task should appear if we filter to a non-draft state.
      const noneShipped = await c.call("maestro_task_list", { plan_id: planId, state: "shipped" });
      const noneItems = (noneShipped.body as { items: unknown[] }).items;
      expect(noneItems.length).toBe(0);
    }
  });
});
