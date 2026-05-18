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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-verify-e2e-"));
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

let slugSeq = 0;
async function setupClaimedTask(dir: string): Promise<string> {
  const slug = `vship-${++slugSeq}`;
  await runCompiled(["spec", "new", slug], dir);
  const created = await runCompiled(["task", "from-spec", `.maestro/specs/${slug}.md`], dir);
  const id = (created.stdout.match(/^(tsk-\S+)/) ?? [])[1];
  await runCompiled(["task", "claim", id!], dir);
  return id!;
}

describe("maestro task verify / ship", () => {
  it("verify PASS auto-advances claimed -> verifying -> ready, ship -> shipped", async () => {
    await mkdir(join(tmpDir, "src/service"), { recursive: true });
    await writeFile(join(tmpDir, "src/service/clean.ts"), `export const X = 1;\n`);

    const id = await setupClaimedTask(tmpDir);
    const verifyResult = await runCompiled(["task", "verify", id], tmpDir);
    expect(verifyResult.exitCode).toBe(0);
    expect(verifyResult.stdout).toContain("verified -> ready (PASS)");

    const taskText = await readFile(join(tmpDir, ".maestro/tasks/tasks.jsonl"), "utf8");
    const taskRow = JSON.parse(taskText.trim()) as { state: string };
    expect(taskRow.state).toBe("ready");

    const rows = await readEvidenceRows(tmpDir);
    const transitions = rows.filter((r) => r.kind === "transition");
    expect(transitions.length).toBe(4);
    expect(transitions[2]).toMatchObject({
      from_state: "claimed",
      to_state: "verifying",
      trigger_verb: "task:verify",
    });
    expect(transitions[3]).toMatchObject({
      from_state: "verifying",
      to_state: "ready",
      trigger_verb: "task:verify",
      verdict: "PASS",
    });

    const shipResult = await runCompiled(
      ["task", "ship", id, "--pr-url", "https://example.test/pr/42"],
      tmpDir,
    );
    expect(shipResult.exitCode).toBe(0);
    expect(shipResult.stdout).toContain("shipped");
    expect(shipResult.stdout).toContain("https://example.test/pr/42");

    const finalRows = await readEvidenceRows(tmpDir);
    const lastTransition = finalRows
      .filter((r) => r.kind === "transition")
      .at(-1) as Record<string, unknown>;
    expect(lastTransition).toMatchObject({
      from_state: "ready",
      to_state: "shipped",
      trigger_verb: "task:ship",
      verdict: "PASS",
    });

    const finalTaskText = await readFile(join(tmpDir, ".maestro/tasks/tasks.jsonl"), "utf8");
    const finalTask = JSON.parse(finalTaskText.trim()) as {
      state: string;
      pr_url?: string;
      merged_at?: string;
    };
    expect(finalTask.state).toBe("shipped");
    expect(finalTask.pr_url).toBe("https://example.test/pr/42");
    expect(finalTask.merged_at).toBeDefined();
  });

  it("verify FAIL keeps state at verifying and emits lint-violation rows tagged with task_id", async () => {
    await mkdir(join(tmpDir, "src/service"), { recursive: true });
    await writeFile(
      join(tmpDir, "src/service/bad.ts"),
      `export function tick() { setInterval(() => null, 1000); }\n`,
    );

    const id = await setupClaimedTask(tmpDir);
    const result = await runCompiled(["task", "verify", id], tmpDir);
    expect(result.exitCode).toBe(1);
    expect(result.stdout).toContain("verify FAIL");
    expect(result.stdout).toContain("passive-harness");

    const taskText = await readFile(join(tmpDir, ".maestro/tasks/tasks.jsonl"), "utf8");
    const taskRow = JSON.parse(taskText.trim()) as { state: string };
    expect(taskRow.state).toBe("verifying");

    const rows = await readEvidenceRows(tmpDir);
    const lintRows = rows.filter((r) => r.kind === "lint-violation");
    expect(lintRows.length).toBe(1);
    expect(lintRows[0]).toMatchObject({
      kind: "lint-violation",
      task_id: id,
      rule_id: "passive-harness",
      severity: "error",
    });

    const transitions = rows.filter((r) => r.kind === "transition");
    const verifyEntries = transitions.filter(
      (t) => (t as { trigger_verb?: string }).trigger_verb === "task:verify",
    );
    expect(verifyEntries.length).toBe(1);
    expect(verifyEntries[0]).toMatchObject({
      from_state: "claimed",
      to_state: "verifying",
    });
  });

  it("ship fails when source state is not ready", async () => {
    const id = await setupClaimedTask(tmpDir);
    const result = await runCompiled(["task", "ship", id], tmpDir);
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("Invalid task transition");
  });

  it("hot-path aliases `verify` and `ship` work at the top level", async () => {
    await mkdir(join(tmpDir, "src/service"), { recursive: true });
    await writeFile(join(tmpDir, "src/service/clean.ts"), `export const X = 1;\n`);

    const id = await setupClaimedTask(tmpDir);
    const v = await runCompiled(["verify", id], tmpDir);
    expect(v.exitCode).toBe(0);
    expect(v.stdout).toContain("PASS");

    const s = await runCompiled(["ship", id], tmpDir);
    expect(s.exitCode).toBe(0);
    expect(s.stdout).toContain("shipped");
  });
});
