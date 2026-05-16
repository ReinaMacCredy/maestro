import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { GitWorktreeStore } from "@/repo/git-worktree-store.adapter.js";
import type { ProcessRunnerPort } from "@/repo/process-runner.port.js";
import {
  WorktreeAlreadyExistsError,
  WorktreeCreateFailedError,
} from "@/repo/worktree-store.port.js";

function makeRunner(
  exitCode = 0,
  stderr = "",
): { runner: ProcessRunnerPort; calls: string[] } {
  const calls: string[] = [];
  return {
    calls,
    runner: {
      async run(cmd) {
        calls.push(cmd);
        return { stdout: "", stderr, exitCode };
      },
    },
  };
}

describe("GitWorktreeStore", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-worktree-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("invokes `git worktree add` and persists the record under .maestro/worktrees/<task-id>.json", async () => {
    const { runner, calls } = makeRunner();
    const FROZEN = new Date("2026-05-15T10:00:00.000Z");
    const store = new GitWorktreeStore({
      repoRoot: root,
      processRunner: runner,
      clock: () => FROZEN,
    });
    const record = await store.create({ task_id: "tsk-1", slug: "demo-task" });

    expect(record.task_id).toBe("tsk-1");
    expect(record.slug).toBe("demo-task");
    expect(record.branch).toBe("feat/demo-task");
    expect(record.base_branch).toBe("main");
    expect(record.path).toBe(join(dirname(root), `${basename(root)}-tsk-1`));
    expect(record.created_at).toBe(FROZEN.toISOString());
    expect(calls.length).toBe(1);
    expect(calls[0]).toContain("git -C");
    expect(calls[0]).toContain("worktree add -b");

    const raw = await readFile(
      join(root, ".maestro/worktrees/tsk-1.json"),
      "utf8",
    );
    const parsed = JSON.parse(raw) as { task_id: string; slug: string };
    expect(parsed.task_id).toBe("tsk-1");
    expect(parsed.slug).toBe("demo-task");
  });

  it("get returns undefined for unknown tasks and the record when present", async () => {
    const { runner } = makeRunner();
    const store = new GitWorktreeStore({ repoRoot: root, processRunner: runner });
    expect(await store.get("tsk-missing")).toBeUndefined();
    await store.create({ task_id: "tsk-2", slug: "another" });
    const found = await store.get("tsk-2");
    expect(found?.slug).toBe("another");
  });

  it("list returns every record in the state dir", async () => {
    const { runner } = makeRunner();
    const store = new GitWorktreeStore({ repoRoot: root, processRunner: runner });
    expect(await store.list()).toEqual([]);
    await store.create({ task_id: "tsk-a", slug: "a" });
    await store.create({ task_id: "tsk-b", slug: "b" });
    const list = await store.list();
    expect(list.length).toBe(2);
    expect(list.map((r) => r.task_id).sort()).toEqual(["tsk-a", "tsk-b"]);
  });

  it("throws WorktreeAlreadyExistsError when create is called twice for the same task", async () => {
    const { runner } = makeRunner();
    const store = new GitWorktreeStore({ repoRoot: root, processRunner: runner });
    await store.create({ task_id: "tsk-dup", slug: "dup" });
    await expect(
      store.create({ task_id: "tsk-dup", slug: "dup" }),
    ).rejects.toBeInstanceOf(WorktreeAlreadyExistsError);
  });

  it("throws WorktreeCreateFailedError when git exits non-zero and does not persist a record", async () => {
    const { runner } = makeRunner(128, "fatal: invalid reference: main");
    const store = new GitWorktreeStore({ repoRoot: root, processRunner: runner });
    await expect(
      store.create({ task_id: "tsk-bad", slug: "bad" }),
    ).rejects.toBeInstanceOf(WorktreeCreateFailedError);
    expect(await store.get("tsk-bad")).toBeUndefined();
  });

  it("honors custom branch_prefix and base_branch", async () => {
    const { runner, calls } = makeRunner();
    const store = new GitWorktreeStore({ repoRoot: root, processRunner: runner });
    await store.create({
      task_id: "tsk-3",
      slug: "fix-thing",
      base_branch: "develop",
      branch_prefix: "fix",
    });
    expect(calls[0]).toContain("fix/fix-thing");
    expect(calls[0]).toContain("develop");
  });
});
