import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  buildCompiledCli,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

const ARCHITECTURE_YAML = `version: 1
forward_only: true
layers:
  - types
  - config
  - repo
  - service
  - runtime
  - ui
cross_cutting:
  - providers
lint_scope:
  - "src/service/**/*.ts"
passive_harness:
  forbidden_patterns:
    - setInterval
`;

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-v2-verify-verdict-"));
  await initGitRepo(tmpDir);
  await mkdir(join(tmpDir, "docs"), { recursive: true });
  await writeFile(join(tmpDir, "docs/architecture.yaml"), ARCHITECTURE_YAML);
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

async function readTaskState(dir: string, taskId: string): Promise<string> {
  const text = await readFile(join(dir, ".maestro/tasks/tasks.jsonl"), "utf8");
  for (const line of text.trim().split("\n")) {
    const t = JSON.parse(line) as { id: string; state: string };
    if (t.id === taskId) return t.state;
  }
  throw new Error(`task ${taskId} not in tasks.jsonl`);
}

let slugSeq = 0;
async function setupClaimedTask(dir: string): Promise<string> {
  const slug = `vv-${++slugSeq}`;
  await runCompiled(["spec", "new", slug], dir);
  const created = await runCompiled(["task", "from-spec", `.maestro/specs/${slug}.md`], dir);
  const id = (created.stdout.match(/^(tsk-\S+)/) ?? [])[1]!;
  await runCompiled(["claim", id], dir);
  return id;
}

describe("maestro task verify --verdict (v2)", () => {
  it("--verdict human keeps the task at verifying and exits with code 2", async () => {
    const id = await setupClaimedTask(tmpDir);
    const result = await runCompiled(
      ["verify", id, "--verdict", "human", "--reason", "design needs review"],
      tmpDir,
    );
    expect(result.exitCode).toBe(2);
    expect(result.stdout).toContain(`${id} verify HUMAN: design needs review`);
    expect(await readTaskState(tmpDir, id)).toBe("verifying");

    const rows = await readEvidenceRows(tmpDir);
    const humanRow = rows.find(
      (r) =>
        r.kind === "transition" &&
        (r as { verdict?: string }).verdict === "HUMAN" &&
        (r as { task_id?: string }).task_id === id,
    );
    expect(humanRow).toMatchObject({
      from_state: "verifying",
      to_state: "verifying",
      verdict: "HUMAN",
      reason: "design needs review",
    });
  });

  it("--verdict block transitions the task to blocked and exits with code 3", async () => {
    const id = await setupClaimedTask(tmpDir);
    const result = await runCompiled(
      ["verify", id, "--verdict", "block", "--reason", "upstream outage"],
      tmpDir,
    );
    expect(result.exitCode).toBe(3);
    expect(result.stdout).toContain(`${id} verify BLOCK -> blocked: upstream outage`);
    expect(await readTaskState(tmpDir, id)).toBe("blocked");

    const rows = await readEvidenceRows(tmpDir);
    const blockRow = rows.find(
      (r) =>
        r.kind === "transition" &&
        (r as { verdict?: string }).verdict === "BLOCK" &&
        (r as { task_id?: string }).task_id === id,
    );
    expect(blockRow).toMatchObject({
      from_state: "verifying",
      to_state: "blocked",
      verdict: "BLOCK",
      reason: "upstream outage",
    });
  });

  it("--verdict without --reason exits 1 with a reason-required error", async () => {
    const id = await setupClaimedTask(tmpDir);
    const result = await runCompiled(["verify", id, "--verdict", "human"], tmpDir);
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("requires --reason");
    expect(await readTaskState(tmpDir, id)).toBe("claimed");
  });

  it("--verdict with an invalid value exits 1 with a clear error", async () => {
    const id = await setupClaimedTask(tmpDir);
    const result = await runCompiled(
      ["verify", id, "--verdict", "maybe", "--reason", "test"],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("must be 'human' or 'block'");
  });

  it("PASS path (no --verdict) still works and exits 0", async () => {
    const id = await setupClaimedTask(tmpDir);
    const result = await runCompiled(["verify", id], tmpDir);
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain(`${id} verified -> ready (PASS)`);
    expect(await readTaskState(tmpDir, id)).toBe("ready");
  });
});
