import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  buildCompiledCli,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-v2-task-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function readEvidenceRows(dir: string): Promise<readonly Record<string, unknown>[]> {
  const evidenceDir = join(dir, ".maestro/evidence");
  let entries: string[];
  try {
    entries = await readdir(evidenceDir);
  } catch {
    return [];
  }
  const files = entries.filter((f) => f.endsWith(".jsonl")).sort();
  const rows: Record<string, unknown>[] = [];
  for (const f of files) {
    const text = await readFile(join(evidenceDir, f), "utf8");
    for (const line of text.split("\n")) {
      if (line.length === 0) continue;
      rows.push(JSON.parse(line));
    }
  }
  return rows;
}

describe("maestro task from-spec + task claim (v2)", () => {
  it("creates a task in draft and emits a kind=transition evidence row", async () => {
    await runCompiled(["spec", "new", "demo-task"], tmpDir);
    const result = await runCompiled(
      ["task", "from-spec", ".maestro/specs/demo-task.md"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toMatch(/^tsk-\S+ draft \(demo-task\)$/m);

    const text = await readFile(join(tmpDir, ".maestro/tasks/tasks.jsonl"), "utf8");
    const lines = text.trim().split("\n");
    expect(lines.length).toBe(1);
    const task = JSON.parse(lines[0]!) as { state: string; slug: string };
    expect(task.state).toBe("draft");
    expect(task.slug).toBe("demo-task");

    const rows = await readEvidenceRows(tmpDir);
    expect(rows.length).toBe(1);
    expect(rows[0]).toMatchObject({
      kind: "transition",
      from_state: null,
      to_state: "draft",
      trigger_verb: "task:from-spec",
    });
  });

  it("claim transitions draft -> claimed and writes a second evidence row", async () => {
    await runCompiled(["spec", "new", "claim-me"], tmpDir);
    const created = await runCompiled(
      ["task", "from-spec", ".maestro/specs/claim-me.md"],
      tmpDir,
    );
    const taskId = (created.stdout.match(/^(tsk-\S+)/) ?? [])[1];
    expect(taskId).toBeDefined();
    const claim = await runCompiled(
      ["task", "claim", taskId!, "--agent", "agent-a"],
      tmpDir,
    );
    expect(claim.exitCode).toBe(0);
    expect(claim.stdout).toContain("claimed by agent-a");

    const text = await readFile(join(tmpDir, ".maestro/tasks/tasks.jsonl"), "utf8");
    const task = JSON.parse(text.trim().split("\n")[0]!) as {
      state: string;
      assignee?: string;
      claimed_at?: string;
    };
    expect(task.state).toBe("claimed");
    expect(task.assignee).toBe("agent-a");
    expect(task.claimed_at).toBeDefined();

    const rows = await readEvidenceRows(tmpDir);
    expect(rows.length).toBe(2);
    expect(rows[1]).toMatchObject({
      kind: "transition",
      from_state: "draft",
      to_state: "claimed",
      trigger_verb: "task:claim",
      agent_id: "agent-a",
    });
  });

  it("hot-path alias `claim` works at the top level", async () => {
    await runCompiled(["spec", "new", "alias-claim"], tmpDir);
    const created = await runCompiled(
      ["task", "from-spec", ".maestro/specs/alias-claim.md"],
      tmpDir,
    );
    const taskId = (created.stdout.match(/^(tsk-\S+)/) ?? [])[1];
    const claim = await runCompiled(["claim", taskId!], tmpDir);
    expect(claim.exitCode).toBe(0);
    expect(claim.stdout).toContain("claimed");
  });

  it("claim on an already-claimed task fails with TaskTransitionError", async () => {
    await runCompiled(["spec", "new", "double-claim"], tmpDir);
    const created = await runCompiled(
      ["task", "from-spec", ".maestro/specs/double-claim.md"],
      tmpDir,
    );
    const taskId = (created.stdout.match(/^(tsk-\S+)/) ?? [])[1];
    await runCompiled(["task", "claim", taskId!], tmpDir);
    const second = await runCompiled(["task", "claim", taskId!], tmpDir);
    expect(second.exitCode).toBe(1);
    expect(second.stderr).toContain("Invalid task transition");
  });
});
