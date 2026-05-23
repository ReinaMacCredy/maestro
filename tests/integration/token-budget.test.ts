/**
 * Token-budget regression guard.
 *
 * Spawns each agent-facing list verb in default and --full modes and
 * asserts:
 *   1. default payload is strictly smaller than --full payload
 *   2. default items contain only the summary projection keys
 *
 * This locks down the doctrine documented in docs/token-budget.md. Drift
 * in projection helpers (`summarizeTask`, `summarizeEvidence`,
 * `summarizeHandoff`) or in the centralized stringify path
 * (`stringifyForOutput`) fails this test loudly.
 */
import { afterAll, beforeAll, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  DIST_CLI,
  buildCompiledCli,
  BUILD_TIMEOUT_MS,
} from "../helpers/run-compiled-cli.js";
import { runCommand } from "../helpers/command-runner.js";

const FIXED_NOW = "2026-05-15T12:00:00.000Z";

let projectRoot: string;

async function spawn(args: readonly string[]): Promise<{ stdout: string; exitCode: number }> {
  const result = await runCommand([DIST_CLI, ...args], projectRoot);
  return { stdout: result.stdout, exitCode: result.exitCode };
}

async function seedTask(): Promise<void> {
  const task = {
    id: "tsk-aaaaaa-bbbbbb",
    slug: "demo-task",
    title: "Demo task with a non-trivial title for byte comparison",
    state: "draft",
    spec_path: ".maestro/specs/demo.md",
    mission_id: "mis-demo-001",
    blocked_by: [],
    created_at: FIXED_NOW,
    updated_at: FIXED_NOW,
  };
  await writeFile(
    join(projectRoot, ".maestro/tasks/tasks.jsonl"),
    `${JSON.stringify(task)}\n`,
    "utf8",
  );
}

async function seedEvidence(): Promise<void> {
  // Evidence storage layout: .maestro/evidence/<task-id>/<evidence-id>.json
  // (one file per row). Both ids must match their regex patterns from
  // src/features/evidence/domain/evidence-id.ts and src/types/task.ts.
  const evidenceId = "evd-1715763600000-abcdef";
  const record = {
    schema_version: 3,
    id: evidenceId,
    task_id: "tsk-aaaaaa-bbbbbb",
    session_id: "sess-aaaaaa",
    kind: "command",
    witness_level: "witnessed-by-maestro",
    created_at: FIXED_NOW,
    payload: {
      command: "bun test some-suite",
      exit: 0,
      log_path: "/tmp/maestro-test-log.txt",
      duration_ms: 1234,
    },
  };
  await writeFile(
    join(projectRoot, `.maestro/evidence/tsk-aaaaaa-bbbbbb/${evidenceId}.json`),
    `${JSON.stringify(record, null, 2)}\n`,
    "utf8",
  );
}

async function seedHandoff(): Promise<void> {
  const envelope = {
    id: "hnd-aaaaaa-bbbbbb",
    task_id: "tsk-aaaaaa-bbbbbb",
    trigger_verb: "task:claim",
    created_at: FIXED_NOW,
    agent_id: "agent-test-suite",
    to_agent: "codex",
    worktree_path: "/tmp/maestro-test-worktree",
    spec_path: ".maestro/specs/demo.md",
    reason: "needs human review of approach before continuing",
  };
  await writeFile(
    join(projectRoot, ".maestro/handoffs/hnd-aaaaaa-bbbbbb.json"),
    `${JSON.stringify(envelope, null, 2)}\n`,
    "utf8",
  );
}

beforeAll(async () => {
  await buildCompiledCli();
  projectRoot = await mkdtemp(join(tmpdir(), "maestro-token-budget-"));
  await mkdir(join(projectRoot, ".maestro/tasks"), { recursive: true });
  await mkdir(join(projectRoot, ".maestro/evidence/tsk-aaaaaa-bbbbbb"), {
    recursive: true,
  });
  await mkdir(join(projectRoot, ".maestro/handoffs"), { recursive: true });
  await runCommand(["git", "init", "-b", "main"], projectRoot);
  await seedTask();
  await seedEvidence();
  await seedHandoff();
}, BUILD_TIMEOUT_MS);

afterAll(async () => {
  await rm(projectRoot, { recursive: true, force: true });
});

describe("token-budget contract: default is smaller than --full", () => {
  it("task list", async () => {
    const def = await spawn(["task", "list", "--json"]);
    const full = await spawn(["task", "list", "--json", "--full", "--all"]);
    expect(def.exitCode).toBe(0);
    expect(full.exitCode).toBe(0);
    expect(def.stdout.length).toBeLessThan(full.stdout.length);
  });

  it("evidence list", async () => {
    const def = await spawn(["evidence", "list", "--json"]);
    const full = await spawn(["evidence", "list", "--json", "--full", "--all"]);
    expect(def.exitCode).toBe(0);
    expect(full.exitCode).toBe(0);
    expect(def.stdout.length).toBeLessThan(full.stdout.length);
  });

  it("handoff list", async () => {
    const def = await spawn(["handoff", "list", "--json"]);
    const full = await spawn(["handoff", "list", "--json", "--full", "--all"]);
    expect(def.exitCode).toBe(0);
    expect(full.exitCode).toBe(0);
    expect(def.stdout.length).toBeLessThan(full.stdout.length);
  });
});

describe("token-budget contract: default emits only summary keys", () => {
  it("task list default omits detail timestamps and paths", async () => {
    const { stdout } = await spawn(["task", "list", "--json"]);
    const parsed = JSON.parse(stdout) as { items: Record<string, unknown>[] };
    expect(parsed.items.length).toBeGreaterThan(0);
    const item = parsed.items[0]!;
    expect("created_at" in item).toBe(false);
    expect("updated_at" in item).toBe(false);
    expect("spec_path" in item).toBe(false);
    expect("blocked_by" in item).toBe(false);
    expect("blocked_by_count" in item).toBe(true);
  });

  it("evidence list default omits payload and schema_version", async () => {
    const { stdout } = await spawn(["evidence", "list", "--json"]);
    const parsed = JSON.parse(stdout) as { items: Record<string, unknown>[] };
    expect(parsed.items.length).toBeGreaterThan(0);
    const item = parsed.items[0]!;
    expect("payload" in item).toBe(false);
    expect("schema_version" in item).toBe(false);
    expect("kind" in item).toBe(true);
  });

  it("handoff list default omits agent_id, worktree_path, spec_path, reason", async () => {
    const { stdout } = await spawn(["handoff", "list", "--json"]);
    const parsed = JSON.parse(stdout) as { items: Record<string, unknown>[] };
    expect(parsed.items.length).toBeGreaterThan(0);
    const item = parsed.items[0]!;
    expect("agent_id" in item).toBe(false);
    expect("worktree_path" in item).toBe(false);
    expect("spec_path" in item).toBe(false);
    expect("reason" in item).toBe(false);
    expect("picked_up" in item).toBe(true);
    expect(item.to_agent).toBe("codex");
  });
});

describe("token-budget contract: piped --json output is minified", () => {
  it("task list output contains no two-space indent", async () => {
    const { stdout } = await spawn(["task", "list", "--json"]);
    // Pretty-printed output would have lines like `  "id"` — minified
    // output is a single line with no leading whitespace before keys.
    expect(stdout).not.toContain('\n  "');
    expect(stdout).not.toContain('{\n  ');
  });

  it("handoff list output contains no two-space indent", async () => {
    const { stdout } = await spawn(["handoff", "list", "--json"]);
    expect(stdout).not.toContain('\n  "');
  });
});
