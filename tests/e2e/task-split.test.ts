import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  buildCompiledCli,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

interface TaskRow {
  readonly id: string;
  readonly slug: string;
  readonly state: string;
  readonly blocked_by: readonly string[];
  readonly parent_id?: string;
  readonly assignee?: string;
}

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-split-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function readTaskRows(dir: string): Promise<readonly TaskRow[]> {
  const text = await readFile(join(dir, ".maestro/tasks/tasks.jsonl"), "utf8");
  return text
    .trim()
    .split("\n")
    .filter((l) => l.length > 0)
    .map((l) => JSON.parse(l) as TaskRow);
}

let slugSeq = 0;
async function setupClaimedTask(
  dir: string,
  flags: { readonly agent?: string } = {},
): Promise<string> {
  const slug = `splitter-${++slugSeq}`;
  await runCompiled(["spec", "new", slug], dir);
  const created = await runCompiled(
    ["task", "from-spec", `.maestro/specs/${slug}.md`],
    dir,
  );
  const id = (created.stdout.match(/^(tsk-\S+)/) ?? [])[1];
  const claimArgs = ["task", "claim", id!];
  if (flags.agent !== undefined) claimArgs.push("--agent", flags.agent);
  await runCompiled(claimArgs, dir);
  return id!;
}

describe("maestro task split", () => {
  it("splits a claimed parent into N children and chains them sequentially by default", async () => {
    const parentId = await setupClaimedTask(tmpDir);
    const result = await runCompiled(
      ["task", "split", parentId, "alpha", "beta", "gamma"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);

    const childLines = result.stdout.split("\n").filter((l) => l.startsWith("tsk-"));
    expect(childLines).toHaveLength(3);
    for (const line of childLines) {
      expect(line).toMatch(/^tsk-\S+ draft \(\S+\)$/);
    }

    const rows = await readTaskRows(tmpDir);
    const byId = new Map(rows.map((r) => [r.id, r] as const));
    const parent = byId.get(parentId)!;
    expect(parent).toBeDefined();

    const childIds = childLines.map((l) => l.split(" ")[0]!);
    for (const cid of childIds) {
      expect(parent.blocked_by).toContain(cid);
    }
    const children = childIds.map((cid) => byId.get(cid)!);
    expect(children.every((c) => c.state === "draft")).toBe(true);
    expect(children.every((c) => c.parent_id === parentId)).toBe(true);

    // Sequential chain (no --parallel): first child has empty blocked_by,
    // each subsequent child depends on its predecessor.
    expect(children[0]!.blocked_by).toEqual([]);
    expect(children[1]!.blocked_by).toEqual([children[0]!.id]);
    expect(children[2]!.blocked_by).toEqual([children[1]!.id]);
  });

  it("--parallel gives every child an empty blocked_by", async () => {
    const parentId = await setupClaimedTask(tmpDir);
    const result = await runCompiled(
      ["task", "split", parentId, "--parallel", "one", "two"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);

    const childIds = result.stdout
      .split("\n")
      .filter((l) => l.startsWith("tsk-"))
      .map((l) => l.split(" ")[0]!);
    expect(childIds).toHaveLength(2);

    const rows = await readTaskRows(tmpDir);
    const byId = new Map(rows.map((r) => [r.id, r] as const));
    for (const cid of childIds) {
      expect(byId.get(cid)!.blocked_by).toEqual([]);
    }
  });

  it("refuses to split a task in draft (wrong state)", async () => {
    const slug = `draft-only-${++slugSeq}`;
    await runCompiled(["spec", "new", slug], tmpDir);
    const created = await runCompiled(
      ["task", "from-spec", `.maestro/specs/${slug}.md`],
      tmpDir,
    );
    const id = (created.stdout.match(/^(tsk-\S+)/) ?? [])[1]!;

    const result = await runCompiled(
      ["task", "split", id, "child"],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("Cannot split task");
    expect(result.stderr).toContain("claimed, doing");
  });

  it("--agent must match the parent's claimant", async () => {
    const parentId = await setupClaimedTask(tmpDir, { agent: "agent-A" });
    const result = await runCompiled(
      ["task", "split", parentId, "--agent", "agent-B", "child"],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("assigned to 'agent-A'");
    expect(result.stderr).toContain("not 'agent-B'");
  });
});
