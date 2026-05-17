import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-v2-plan-decompose-"));
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

async function seedSpecifiedPlan(dir: string): Promise<string> {
  await runCompiled(["spec", "new", "demo-heavy", "--mode", "heavy"], dir);
  const created = await runCompiled(
    ["mission", "from-spec", ".maestro/specs/demo-heavy.md"],
    dir,
  );
  const planId = (created.stdout.match(/^(pln-\S+)/) ?? [])[1];
  if (!planId) throw new Error(`could not parse plan id from: ${created.stdout}`);
  return planId;
}

describe("maestro mission decompose (v2)", () => {
  it("creates child tasks linked by mission_id and advances the plan to 'planned'", async () => {
    const planId = await seedSpecifiedPlan(tmpDir);
    const batchPath = join(tmpDir, "batch.json");
    await writeFile(
      batchPath,
      JSON.stringify([
        { title: "First task", slug: "first" },
        { title: "Second task", slug: "second", spec_path: ".maestro/specs/demo-heavy.md" },
        { title: "Third task", slug: "third" },
      ]),
    );

    const result = await runCompiled(
      ["mission", "decompose", planId, "--file", batchPath],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain(`${planId} planned (3 tasks)`);
    expect(result.stdout).toMatch(/tsk-\S+ draft first/);
    expect(result.stdout).toMatch(/tsk-\S+ draft second/);
    expect(result.stdout).toMatch(/tsk-\S+ draft third/);

    const planText = await readFile(join(tmpDir, ".maestro/missions/missions.jsonl"), "utf8");
    const planLines = planText.trim().split("\n");
    expect(planLines.length).toBe(1);
    const plan = JSON.parse(planLines[0]!) as { state: string; id: string };
    expect(plan.id).toBe(planId);
    expect(plan.state).toBe("planned");

    const taskText = await readFile(join(tmpDir, ".maestro/tasks/tasks.jsonl"), "utf8");
    const taskLines = taskText.trim().split("\n");
    expect(taskLines.length).toBe(3);
    const tasks = taskLines.map((l) => JSON.parse(l) as { mission_id?: string; slug: string });
    for (const t of tasks) {
      expect(t.mission_id).toBe(planId);
    }
    const slugs = tasks.map((t) => t.slug).sort();
    expect(slugs).toEqual(["first", "second", "third"]);

    const rows = await readEvidenceRows(tmpDir);
    const missionFromSpec = rows.find(
      (r) => r.trigger_verb === "mission:from-spec",
    );
    const missionDecompose = rows.find(
      (r) => r.trigger_verb === "mission:decompose",
    );
    const taskRows = rows.filter((r) => r.trigger_verb === "task:from-spec");
    expect(missionFromSpec).toBeDefined();
    expect(missionDecompose).toMatchObject({
      kind: "transition",
      mission_id: planId,
      from_state: "specified",
      to_state: "planned",
      trigger_verb: "mission:decompose",
    });
    expect(taskRows.length).toBe(3);
    for (const r of taskRows) {
      expect(r).toMatchObject({
        kind: "transition",
        from_state: null,
        to_state: "draft",
        trigger_verb: "task:from-spec",
      });
    }
  });

  it("reads the batch JSON from stdin when --file is '-'", async () => {
    const planId = await seedSpecifiedPlan(tmpDir);
    const batch = JSON.stringify([{ title: "Solo", slug: "solo" }]);
    const result = await runCompiled(
      ["mission", "decompose", planId, "--file", "-"],
      tmpDir,
      { stdin: batch },
    );
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain(`${planId} planned (1 task)`);
  });

  it("fails with PlanDecomposeBatchEmptyError for an empty array", async () => {
    const planId = await seedSpecifiedPlan(tmpDir);
    const batchPath = join(tmpDir, "empty.json");
    await writeFile(batchPath, "[]");
    const result = await runCompiled(
      ["mission", "decompose", planId, "--file", batchPath],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("at least one task");
  });

  it("fails when the plan is already planned (transition rejected)", async () => {
    const planId = await seedSpecifiedPlan(tmpDir);
    const batchPath = join(tmpDir, "batch.json");
    await writeFile(batchPath, JSON.stringify([{ title: "A", slug: "a" }]));
    const first = await runCompiled(
      ["mission", "decompose", planId, "--file", batchPath],
      tmpDir,
    );
    expect(first.exitCode).toBe(0);

    const batch2Path = join(tmpDir, "batch2.json");
    await writeFile(batch2Path, JSON.stringify([{ title: "B", slug: "b" }]));
    const second = await runCompiled(
      ["mission", "decompose", planId, "--file", batch2Path],
      tmpDir,
    );
    expect(second.exitCode).toBe(1);
    expect(second.stderr).toContain("Invalid mission transition planned -> planned");
  });

  it("fails for a missing plan id", async () => {
    const batchPath = join(tmpDir, "batch.json");
    await writeFile(batchPath, JSON.stringify([{ title: "A", slug: "a" }]));
    const result = await runCompiled(
      ["mission", "decompose", "pln-doesnotexist", "--file", batchPath],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("not found");
  });
});
