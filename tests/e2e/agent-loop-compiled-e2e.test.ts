import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
} from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  BUILD_TIMEOUT_MS,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  expectJson,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-agent-loop-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("agent session loop (plan -> next -> update)", () => {
  it("exposes the plan input schema via --schema", async () => {
    const result = await runCompiled(["task", "plan", "--schema"], tmpDir);
    expect(result.exitCode).toBe(0);
    const schema = expectJson<Record<string, unknown>>(result);
    expect(schema.type).toBe("object");
    const defs = schema.$defs as Record<string, { required: string[] }>;
    expect(defs.BatchTaskInput.required).toEqual(["title"]);
  }, SLOW_CLI_TIMEOUT_MS);

  it("errors when neither --file nor --schema is given", async () => {
    const result = await runCompiled(["task", "plan"], tmpDir);
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr + result.stdout).toContain("--file is required");
  }, SLOW_CLI_TIMEOUT_MS);

  it("plans a batch, claims the ready task via `task next`, and advances it", async () => {
    const planPath = join(tmpDir, "plan.json");
    await writeFile(
      planPath,
      JSON.stringify({
        tasks: [
          { name: "first", title: "Scaffold" },
          { name: "second", title: "Ship", blockedBy: ["first"] },
        ],
      }),
    );
    const plan = await runCompiled(
      ["task", "plan", "--file", planPath, "--json"],
      tmpDir,
    );
    expect(plan.exitCode).toBe(0);
    const planResult = expectJson<{ created: Array<{ name: string; id: string }> }>(plan);
    const firstId = planResult.created.find((t) => t.name === "first")!.id;
    const secondId = planResult.created.find((t) => t.name === "second")!.id;

    const next = await runCompiled(
      ["task", "next", "--session", "codex-loop-a", "--json"],
      tmpDir,
    );
    expect(next.exitCode).toBe(0);
    const nextResult = expectJson<{ task?: { id: string; assignee?: string }; reason?: string }>(next);
    expect(nextResult.task?.id).toBe(firstId);
    expect(nextResult.task?.assignee).toBe("codex-loop-a");

    const again = await runCompiled(
      ["task", "next", "--session", "codex-loop-a", "--json"],
      tmpDir,
    );
    expect(again.exitCode).not.toBe(0);
    expect(again.stderr + again.stdout).toContain("update or unclaim before pulling another");

    const complete = await runCompiled(
      [
        "task",
        "update",
        firstId,
        "--status",
        "completed",
        "--reason",
        "done",
        "--session",
        "codex-loop-a",
        "--json",
      ],
      tmpDir,
    );
    expect(complete.exitCode).toBe(0);

    const pullSecond = await runCompiled(
      ["task", "next", "--session", "codex-loop-a", "--json"],
      tmpDir,
    );
    expect(pullSecond.exitCode).toBe(0);
    const secondResult = expectJson<{ task?: { id: string; assignee?: string } }>(pullSecond);
    expect(secondResult.task?.id).toBe(secondId);
  }, SLOW_CLI_TIMEOUT_MS);

  it("reports `nothing pending` when the queue is empty", async () => {
    const result = await runCompiled(
      ["task", "next", "--session", "codex-loop-empty", "--json"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    const payload = expectJson<{ task?: unknown; reason?: string }>(result);
    expect(payload.task).toBeUndefined();
    expect(payload.reason).toBe("nothing pending");
  }, SLOW_CLI_TIMEOUT_MS);

  it("reports `all blocked` when every pending task is blocked", async () => {
    const planPath = join(tmpDir, "blocked-plan.json");
    await writeFile(
      planPath,
      JSON.stringify({
        tasks: [
          { name: "root", title: "Root" },
          { name: "child", title: "Child", blockedBy: ["root"] },
        ],
      }),
    );
    const plan = await runCompiled(
      ["task", "plan", "--file", planPath, "--json"],
      tmpDir,
    );
    expect(plan.exitCode).toBe(0);
    const planResult = expectJson<{ created: Array<{ name: string; id: string }> }>(plan);
    const rootId = planResult.created.find((t) => t.name === "root")!.id;

    const claim = await runCompiled(
      ["task", "claim", rootId, "--session", "other-owner", "--json"],
      tmpDir,
    );
    expect(claim.exitCode).toBe(0);

    const result = await runCompiled(
      ["task", "next", "--session", "codex-loop-block", "--json"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    const payload = expectJson<{ task?: unknown; reason?: string }>(result);
    expect(payload.task).toBeUndefined();
    expect(payload.reason).toBe("all blocked");
  }, SLOW_CLI_TIMEOUT_MS);
});

describe("handoff discovery surfaces", () => {
  it("returns a JSON array", async () => {
    const result = await runCompiled(["handoff", "list", "--json"], tmpDir);
    expect(result.exitCode).toBe(0);
    const payload = expectJson<unknown[]>(result);
    expect(Array.isArray(payload)).toBe(true);
  }, SLOW_CLI_TIMEOUT_MS);

  it("errors with a helpful message on handoff show <missing>", async () => {
    const result = await runCompiled(["handoff", "show", "swift-otter-9999"], tmpDir);
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr + result.stdout).toContain("Handoff packet not found");
  }, SLOW_CLI_TIMEOUT_MS);
});
