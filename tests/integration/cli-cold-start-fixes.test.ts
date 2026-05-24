import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import {
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runCli } from "../helpers/run-cli.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-cold-start-fixes-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function setupProject(): Promise<void> {
  const result = await runCli(["setup", "--no-git-ok"], tmpDir);
  if (result.exitCode !== 0) {
    throw new Error(
      `setup failed (exit ${result.exitCode}): ${result.stderr}\n${result.stdout}`,
    );
  }
}

async function readHandoffEnvelopes(): Promise<
  Array<Record<string, unknown> & { id: string }>
> {
  const dir = join(tmpDir, ".maestro", "handoffs");
  const entries = await readdir(dir);
  const envelopes: Array<Record<string, unknown> & { id: string }> = [];
  for (const file of entries) {
    if (file.endsWith(".picked_up.json") || !file.endsWith(".json")) continue;
    const text = await readFile(join(dir, file), "utf8");
    envelopes.push(JSON.parse(text));
  }
  return envelopes;
}

async function createTask(slug: string): Promise<string> {
  const specPath = join(".maestro", "specs", `${slug}.md`);
  await runCli(["spec", "new", slug], tmpDir);
  const created = await runCli(["task", "from-spec", specPath], tmpDir);
  const match = created.stdout.match(/(tsk-\S+)/);
  if (!match) {
    throw new Error(
      `expected tsk- id in stdout, got:\n${created.stdout}\n${created.stderr}`,
    );
  }
  return match[1]!;
}

describe("regression: CLI --tool plumbing on claim/block (FIX-3)", () => {
  // Regression: FIX-3 -- task.command.ts previously did not register the
  // `--tool` option on `claim`/`block`. Auto-emitted handoffs always carried
  // `to_agent: undefined`, breaking strict routing in any MCP consumer that
  // filters by recipient.
  it("propagates --tool to the auto-emitted claim handoff's to_agent field", async () => {
    await setupProject();
    const taskId = await createTask("alpha");

    const claim = await runCli(
      ["task", "claim", taskId, "--tool", "codex-bot", "--skip-worktree"],
      tmpDir,
    );
    expect(claim.exitCode).toBe(0);

    const envelopes = await readHandoffEnvelopes();
    const ours = envelopes.find(
      (e) => e.task_id === taskId && e.trigger_verb === "task:claim",
    );
    expect(ours).toBeDefined();
    expect(ours?.to_agent).toBe("codex-bot");
  });

  it("defaults to_agent to 'cli' when --tool is omitted on claim", async () => {
    await setupProject();
    const taskId = await createTask("beta");

    const claim = await runCli(
      ["task", "claim", taskId, "--skip-worktree"],
      tmpDir,
    );
    expect(claim.exitCode).toBe(0);

    const envelopes = await readHandoffEnvelopes();
    const ours = envelopes.find(
      (e) => e.task_id === taskId && e.trigger_verb === "task:claim",
    );
    expect(ours?.to_agent).toBe("cli");
  });

  // Category sibling: block emits a `task:block` handoff that must carry the
  // same plumbing.
  it("propagates --tool to the auto-emitted block handoff's to_agent field", async () => {
    await setupProject();
    const taskId = await createTask("gamma");
    await runCli(
      ["task", "claim", taskId, "--skip-worktree"],
      tmpDir,
    );

    const blocked = await runCli(
      [
        "task",
        "block",
        taskId,
        "--reason",
        "wait on upstream",
        "--tool",
        "codex-bot",
      ],
      tmpDir,
    );
    expect(blocked.exitCode).toBe(0);

    const envelopes = await readHandoffEnvelopes();
    const ours = envelopes.find(
      (e) => e.task_id === taskId && e.trigger_verb === "task:block",
    );
    expect(ours?.to_agent).toBe("codex-bot");
  });
});

describe("regression: status walks up to project root (FIX-4)", () => {
  // Regression: FIX-4 -- the status command used `process.cwd()` directly, so
  // any subdir invocation tripped the "not initialized" guard. `maestro
  // doctor` was already walking up; the two surfaces disagreed on where the
  // project root was, which broke the cold-start init.sh flow.
  it("succeeds from a deeply nested subdirectory", async () => {
    await setupProject();
    const deepDir = join(tmpDir, "src", "features", "deep", "nested");
    await mkdir(deepDir, { recursive: true });

    const result = await runCli(["status"], deepDir);

    expect(result.exitCode).toBe(0);
    expect(result.stderr).not.toContain("not initialized");
    expect(result.stdout).toContain("Maestro health");
  });

  // Boundary: from the project root itself still works.
  it("succeeds from the project root", async () => {
    await setupProject();
    const result = await runCli(["status"], tmpDir);
    expect(result.exitCode).toBe(0);
    expect(result.stderr).not.toContain("not initialized");
  });
});

describe("regression: CLI handoff list parity with MCP (FIX-8/9/10)", () => {
  interface ListPayload {
    items: Array<Record<string, unknown> & { id: string }>;
    total: number;
    limit: number;
    offset: number;
    pagination?: {
      total: number;
      limit: number;
      offset: number;
      hasMore: boolean;
    };
  }

  async function seedTwoHandoffsWithOnePickedUp(): Promise<{
    openId: string;
    pickedId: string;
  }> {
    await setupProject();
    const taskA = await createTask("alpha");
    const taskB = await createTask("beta");
    await runCli(["task", "claim", taskA, "--skip-worktree"], tmpDir);
    await runCli(["task", "claim", taskB, "--skip-worktree"], tmpDir);

    const envelopes = await readHandoffEnvelopes();
    const envA = envelopes.find((e) => e.task_id === taskA);
    const envB = envelopes.find((e) => e.task_id === taskB);
    if (!envA || !envB) throw new Error("expected two handoff envelopes");

    // Mark envA as picked up via the canonical sidecar file shape.
    const pickupPath = join(
      tmpDir,
      ".maestro",
      "handoffs",
      `${envA.id}.picked_up.json`,
    );
    await writeFile(
      pickupPath,
      `${JSON.stringify(
        {
          id: "pkp-test01",
          envelope_id: envA.id,
          picked_up_by: "test",
          picked_up_at: new Date().toISOString(),
        },
        null,
        2,
      )}\n`,
    );

    return { openId: envB.id, pickedId: envA.id };
  }

  // Regression: FIX-8 -- CLI defaulted to "show everything" while MCP
  // defaulted to "open only". The two surfaces returned different sets for an
  // unflagged `list`. The fix unifies both on "open-only" and computes
  // `total` from the visible (post-filter) set so pagination is consistent.
  it("defaults to open-only (picked-up envelopes are hidden) and total reflects the filtered set", async () => {
    const { openId, pickedId } = await seedTwoHandoffsWithOnePickedUp();

    const result = await runCli(["handoff", "list", "--json"], tmpDir);
    expect(result.exitCode).toBe(0);
    const payload = JSON.parse(result.stdout) as ListPayload;

    const ids = payload.items.map((i) => i.id);
    expect(ids).toContain(openId);
    expect(ids).not.toContain(pickedId);
    expect(payload.total).toBe(1);
  });

  it("--include-picked-up surfaces previously-hidden envelopes and total bumps to match", async () => {
    const { openId, pickedId } = await seedTwoHandoffsWithOnePickedUp();

    const result = await runCli(
      ["handoff", "list", "--include-picked-up", "--json"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    const payload = JSON.parse(result.stdout) as ListPayload;

    const ids = payload.items.map((i) => i.id);
    expect(ids).toContain(openId);
    expect(ids).toContain(pickedId);
    expect(payload.total).toBe(2);
  });

  // Regression: FIX-9 -- CLI emitted only `{total, limit, offset}` flat while
  // MCP emitted a nested `pagination` block. Downstream consumers had no
  // single shape to target. The fix emits BOTH so callers can pick either.
  it("--json output carries the legacy flat shape AND the nested pagination block", async () => {
    await seedTwoHandoffsWithOnePickedUp();

    const result = await runCli(["handoff", "list", "--json"], tmpDir);
    const payload = JSON.parse(result.stdout) as ListPayload;

    // Flat fields still present (legacy callers).
    expect(typeof payload.total).toBe("number");
    expect(typeof payload.limit).toBe("number");
    expect(typeof payload.offset).toBe("number");

    // Nested block mirrors them and adds `hasMore`.
    expect(payload.pagination).toBeDefined();
    expect(payload.pagination?.total).toBe(payload.total);
    expect(payload.pagination?.limit).toBe(payload.limit);
    expect(payload.pagination?.offset).toBe(payload.offset);
    expect(typeof payload.pagination?.hasMore).toBe("boolean");
  });

  // Regression: FIX-10 -- the summary projection included a `picked_up` flag
  // but the `--full` path emitted bare envelopes, dropping that bit. Callers
  // doing `list --full --json` couldn't tell open vs picked-up apart.
  it("--full --json includes the picked_up flag on every item", async () => {
    await seedTwoHandoffsWithOnePickedUp();

    const result = await runCli(
      ["handoff", "list", "--full", "--include-picked-up", "--json"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    const payload = JSON.parse(result.stdout) as {
      items: Array<{ envelope: { id: string }; picked_up: boolean }>;
    };

    expect(payload.items.length).toBe(2);
    for (const item of payload.items) {
      expect(typeof item.picked_up).toBe("boolean");
      expect(typeof item.envelope?.id).toBe("string");
    }
    // At least one is true and one is false.
    const pickedFlags = payload.items.map((i) => i.picked_up);
    expect(pickedFlags).toContain(true);
    expect(pickedFlags).toContain(false);
  });
});

describe("regression: task list pagination math (FIX-11)", () => {
  // Regression: FIX-11 -- `--all` computes rawLimit as `tasks.length - offset`.
  // When offset > tasks.length, rawLimit goes negative and was emitted as-is
  // in the JSON `limit` field, breaking downstream pagination math. The fix
  // clamps to `Math.max(rawLimit, 0)`. The smallest failing input is
  // `--all --offset N+k` for k > 0; the boundary case is offset === N.
  async function seedOneTask(): Promise<void> {
    await setupProject();
    await createTask("solo");
  }

  it("returns empty items and clamps limit to 0 when --all --offset is past the end", async () => {
    await seedOneTask();

    const result = await runCli(
      ["task", "list", "--all", "--offset", "10", "--json"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    const payload = JSON.parse(result.stdout) as {
      items: unknown[];
      total: number;
      limit: number;
      offset: number;
    };

    expect(payload.items).toEqual([]);
    expect(payload.total).toBe(1);
    expect(payload.offset).toBe(10);
    expect(payload.limit).toBeGreaterThanOrEqual(0);
  });

  it("boundary: --all --offset exactly equal to tasks.length still emits non-negative limit", async () => {
    await seedOneTask();

    const result = await runCli(
      ["task", "list", "--all", "--offset", "1", "--json"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    const payload = JSON.parse(result.stdout) as {
      items: unknown[];
      limit: number;
      offset: number;
    };

    expect(payload.items).toEqual([]);
    expect(payload.offset).toBe(1);
    expect(payload.limit).toBeGreaterThanOrEqual(0);
  });
});
