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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-block-e2e-"));
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

let slugSeq = 0;
async function setupClaimedTask(dir: string): Promise<string> {
  const slug = `tester-${++slugSeq}`;
  await runCompiled(["spec", "new", slug], dir);
  const created = await runCompiled(
    ["task", "from-spec", `.maestro/specs/${slug}.md`],
    dir,
  );
  const id = (created.stdout.match(/^(tsk-\S+)/) ?? [])[1];
  await runCompiled(["task", "claim", id!], dir);
  return id!;
}

describe("maestro task block / abandon", () => {
  it("block transitions claimed -> blocked and records the reason", async () => {
    const id = await setupClaimedTask(tmpDir);
    const result = await runCompiled(
      ["task", "block", id, "--reason", "waiting for design"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("blocked: waiting for design");

    const text = await readFile(join(tmpDir, ".maestro/tasks/tasks.jsonl"), "utf8");
    const task = JSON.parse(text.trim()) as { state: string; block_reason?: string };
    expect(task.state).toBe("blocked");
    expect(task.block_reason).toBe("waiting for design");

    const rows = await readEvidenceRows(tmpDir);
    expect(rows[rows.length - 1]).toMatchObject({
      kind: "transition",
      from_state: "claimed",
      to_state: "blocked",
      trigger_verb: "task:block",
      reason: "waiting for design",
    });
  });

  it("block requires --reason", async () => {
    const id = await setupClaimedTask(tmpDir);
    const result = await runCompiled(["task", "block", id], tmpDir);
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("--reason");
  });

  it("abandon transitions claimed -> abandoned and is terminal (re-abandon fails)", async () => {
    const id = await setupClaimedTask(tmpDir);
    const result = await runCompiled(
      ["task", "abandon", id, "--reason", "no longer needed"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("abandoned: no longer needed");

    const second = await runCompiled(
      ["task", "abandon", id, "--reason", "again"],
      tmpDir,
    );
    expect(second.exitCode).toBe(1);
    expect(second.stderr).toContain("Invalid task transition");
  });

  it("blocking the last active task auto-pauses the parent mission", async () => {
    await runCompiled(["spec", "new", "pause-flow", "--mode", "heavy"], tmpDir);
    const fromSpec = await runCompiled(
      ["mission", "from-spec", ".maestro/specs/pause-flow.md"],
      tmpDir,
    );
    const missionId = (fromSpec.stdout.match(/^(pln-\S+)/) ?? [])[1];
    expect(missionId).toBeDefined();

    const batchPath = join(tmpDir, "pause-flow-batch.json");
    await writeFile(batchPath, JSON.stringify([{ title: "Only task", slug: "only" }]));
    await runCompiled(["mission", "decompose", missionId!, "--file", batchPath], tmpDir);

    const tasksJson = await readFile(join(tmpDir, ".maestro/tasks/tasks.jsonl"), "utf8");
    const taskId = (JSON.parse(tasksJson.trim()) as { id: string }).id;

    await runCompiled(["task", "claim", taskId], tmpDir);

    const missionAfterClaim = JSON.parse(
      (await readFile(join(tmpDir, ".maestro/missions/missions.jsonl"), "utf8")).trim(),
    ) as { state: string };
    expect(missionAfterClaim.state).toBe("in-progress");

    await runCompiled(["task", "block", taskId, "--reason", "waiting"], tmpDir);

    const missionAfterBlock = JSON.parse(
      (await readFile(join(tmpDir, ".maestro/missions/missions.jsonl"), "utf8")).trim(),
    ) as { state: string };
    expect(missionAfterBlock.state).toBe("paused");

    const rows = await readEvidenceRows(tmpDir);
    const pauseRow = rows.find(
      (r) => r.to_state === "paused" && r.trigger_verb === "mission:auto-pause",
    );
    expect(pauseRow).toBeDefined();
  });

  it("hot-path aliases block and abandon work at top level", async () => {
    const id = await setupClaimedTask(tmpDir);
    const blockRes = await runCompiled(["block", id, "--reason", "alias"], tmpDir);
    expect(blockRes.exitCode).toBe(0);

    const id2 = await setupClaimedTask(tmpDir);
    const abandonRes = await runCompiled(["abandon", id2, "--reason", "alias"], tmpDir);
    expect(abandonRes.exitCode).toBe(0);
  });
});
