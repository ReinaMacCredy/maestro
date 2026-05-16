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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-v2-plan-from-spec-"));
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

describe("maestro plan from-spec + plan show (v2)", () => {
  it("plan from-spec on a heavy-mode spec creates a plan in 'specified' and emits one transition row", async () => {
    await runCompiled(["spec", "new", "demo-heavy", "--mode", "heavy"], tmpDir);
    const result = await runCompiled(
      ["plan", "from-spec", ".maestro/specs/demo-heavy.md"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toMatch(/^pln-\S+ specified \(demo-heavy\)$/m);

    const text = await readFile(join(tmpDir, ".maestro/plans/plans.jsonl"), "utf8");
    const lines = text.trim().split("\n");
    expect(lines.length).toBe(1);
    const plan = JSON.parse(lines[0]) as { state: string; slug: string };
    expect(plan.state).toBe("specified");
    expect(plan.slug).toBe("demo-heavy");

    const rows = await readEvidenceRows(tmpDir);
    expect(rows.length).toBe(1);
    expect(rows[0]).toMatchObject({
      kind: "transition",
      from_state: null,
      to_state: "specified",
      trigger_verb: "plan:from-spec",
    });
  });

  it("plan from-spec on a light-mode spec fails with PlanRequiresHeavyModeError", async () => {
    await runCompiled(["spec", "new", "demo-light"], tmpDir);
    const result = await runCompiled(
      ["plan", "from-spec", ".maestro/specs/demo-light.md"],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("plan from-spec requires mode: heavy");
  });

  it("plan show emits text by default and JSON with --json", async () => {
    await runCompiled(["spec", "new", "demo-heavy", "--mode", "heavy"], tmpDir);
    const created = await runCompiled(
      ["plan", "from-spec", ".maestro/specs/demo-heavy.md"],
      tmpDir,
    );
    const planId = (created.stdout.match(/^(pln-\S+)/) ?? [])[1];
    expect(planId).toBeDefined();

    const text = await runCompiled(["plan", "show", planId!], tmpDir);
    expect(text.exitCode).toBe(0);
    expect(text.stdout).toContain(`${planId} specified`);
    expect(text.stdout).toContain("(no child tasks yet)");

    const json = await runCompiled(["plan", "show", planId!, "--json"], tmpDir);
    expect(json.exitCode).toBe(0);
    const parsed = JSON.parse(json.stdout) as { plan: { id: string; state: string }; tasks: unknown[] };
    expect(parsed.plan.id).toBe(planId!);
    expect(parsed.plan.state).toBe("specified");
    expect(parsed.tasks).toEqual([]);
  });

  it("plan show on a missing id fails with ExecPlanNotFoundError", async () => {
    const result = await runCompiled(["plan", "show", "pln-doesnotexist"], tmpDir);
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("not found");
  });
});
