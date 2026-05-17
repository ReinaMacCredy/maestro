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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-v2-plan-auto-advance-"));
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

async function readPlanState(dir: string, planId: string): Promise<string> {
  const text = await readFile(join(dir, ".maestro/missions/plans.jsonl"), "utf8");
  for (const line of text.trim().split("\n")) {
    const p = JSON.parse(line) as { id: string; state: string };
    if (p.id === planId) return p.state;
  }
  throw new Error(`plan ${planId} not in plans.jsonl`);
}

async function readTasks(dir: string): Promise<readonly { id: string; slug: string }[]> {
  const text = await readFile(join(dir, ".maestro/tasks/tasks.jsonl"), "utf8");
  return text.trim().split("\n").map((l) => JSON.parse(l) as { id: string; slug: string });
}

describe("maestro plan auto-advance (v2 ADR-0011)", () => {
  it("planned -> in-progress on first task claim; in-progress -> completed when every child is shipped", async () => {
    // Setup: heavy spec, plan from-spec, decompose into 3 tasks.
    await runCompiled(["spec", "new", "demo-heavy", "--mode", "heavy"], tmpDir);
    const created = await runCompiled(
      ["mission", "from-spec", ".maestro/specs/demo-heavy.md"],
      tmpDir,
    );
    const planId = (created.stdout.match(/^(pln-\S+)/) ?? [])[1]!;

    const batchPath = join(tmpDir, "batch.json");
    await writeFile(
      batchPath,
      JSON.stringify([
        { title: "First", slug: "first" },
        { title: "Second", slug: "second" },
        { title: "Third", slug: "third" },
      ]),
    );
    const decomposed = await runCompiled(
      ["mission", "decompose", planId, "--file", batchPath],
      tmpDir,
    );
    expect(decomposed.exitCode).toBe(0);
    expect(await readPlanState(tmpDir, planId)).toBe("planned");

    const tasks = await readTasks(tmpDir);
    const [t1, t2, t3] = tasks;
    expect(t1).toBeDefined();
    expect(t2).toBeDefined();
    expect(t3).toBeDefined();

    // First claim: plan should auto-start to in-progress.
    const claim1 = await runCompiled(["claim", t1!.id], tmpDir);
    expect(claim1.exitCode).toBe(0);
    expect(await readPlanState(tmpDir, planId)).toBe("in-progress");

    // Second claim: plan stays in-progress, no new plan transition row.
    const beforeRows = (await readEvidenceRows(tmpDir)).length;
    const claim2 = await runCompiled(["claim", t2!.id], tmpDir);
    expect(claim2.exitCode).toBe(0);
    expect(await readPlanState(tmpDir, planId)).toBe("in-progress");
    const afterRows = (await readEvidenceRows(tmpDir)).length;
    // Exactly one new row: the task:claim transition. No plan:auto-* emitted.
    expect(afterRows).toBe(beforeRows + 1);

    // Walk each task to shipped. First two task ships should keep the plan
    // in-progress; the third should auto-complete it.
    for (const task of [t1, t2, t3]) {
      // claimed -> doing -> verifying -> ready (manual transitions are not in v2 yet for doing/verifying;
      // jump straight by claiming if not already claimed, then ship after verify).
    }

    // task t3 hasn't been claimed yet — claim it.
    await runCompiled(["claim", t3!.id], tmpDir);

    // To reach 'ready' we go through verify (verifying -> ready PASS). For the
    // dogfood test, do that for each task. The v2 verify usecase auto-advances
    // verifying -> ready on PASS; tasks aren't in 'verifying' here, so we need
    // a way to put them there. The current usecase set doesn't expose a manual
    // doing/verifying transition, but verify itself accepts any non-terminal
    // state and asserts the verifying transition is allowed before re-running.
    // Use the verify verb directly: if the task isn't in 'verifying' it
    // transitions claimed -> verifying via the usecase's relaxation. The verify
    // dogfood path already exercised this for claimed tasks.
    for (const task of [t1, t2, t3]) {
      const v = await runCompiled(["verify", task!.id], tmpDir);
      expect(v.exitCode).toBe(0);
      expect(v.stdout).toContain("verified -> ready");
    }

    // Ship in order; only the final ship should trigger plan auto-complete.
    const ship1 = await runCompiled(["ship", t1!.id], tmpDir);
    expect(ship1.exitCode).toBe(0);
    expect(await readPlanState(tmpDir, planId)).toBe("in-progress");
    const ship2 = await runCompiled(["ship", t2!.id], tmpDir);
    expect(ship2.exitCode).toBe(0);
    expect(await readPlanState(tmpDir, planId)).toBe("in-progress");
    const ship3 = await runCompiled(["ship", t3!.id], tmpDir);
    expect(ship3.exitCode).toBe(0);
    expect(await readPlanState(tmpDir, planId)).toBe("completed");

    // Evidence sanity: exactly one mission:auto-start and one mission:auto-complete.
    const rows = await readEvidenceRows(tmpDir);
    const autoStart = rows.filter((r) => r.trigger_verb === "mission:auto-start");
    const autoComplete = rows.filter((r) => r.trigger_verb === "mission:auto-complete");
    expect(autoStart.length).toBe(1);
    expect(autoComplete.length).toBe(1);
    expect(autoStart[0]).toMatchObject({
      mission_id: planId,
      from_state: "planned",
      to_state: "in-progress",
    });
    expect(autoComplete[0]).toMatchObject({
      mission_id: planId,
      from_state: "in-progress",
      to_state: "completed",
    });
  });

  it("abandoning the last non-terminal sibling auto-completes the plan", async () => {
    await runCompiled(["spec", "new", "demo-heavy", "--mode", "heavy"], tmpDir);
    const created = await runCompiled(
      ["mission", "from-spec", ".maestro/specs/demo-heavy.md"],
      tmpDir,
    );
    const planId = (created.stdout.match(/^(pln-\S+)/) ?? [])[1]!;

    const batchPath = join(tmpDir, "batch.json");
    await writeFile(
      batchPath,
      JSON.stringify([
        { title: "A", slug: "a" },
        { title: "B", slug: "b" },
      ]),
    );
    await runCompiled(["mission", "decompose", planId, "--file", batchPath], tmpDir);

    const tasks = await readTasks(tmpDir);
    const [a, b] = tasks;

    // Claim 'a' (plan -> in-progress), then abandon 'a' and 'b' to terminal.
    await runCompiled(["claim", a!.id], tmpDir);
    expect(await readPlanState(tmpDir, planId)).toBe("in-progress");
    await runCompiled(["abandon", a!.id, "--reason", "no longer needed"], tmpDir);
    expect(await readPlanState(tmpDir, planId)).toBe("in-progress"); // b still draft
    await runCompiled(["abandon", b!.id, "--reason", "out of scope"], tmpDir);
    expect(await readPlanState(tmpDir, planId)).toBe("completed");
  });
});
