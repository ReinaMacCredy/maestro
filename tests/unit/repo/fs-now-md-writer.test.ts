import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsNowMdWriter } from "@/repo/fs-now-md-writer.adapter.js";
import type { Task } from "@/types/task.js";

const TASK: Task = {
  id: "tsk-x-1",
  slug: "demo",
  title: "demo task",
  state: "draft",
  blocked_by: [],
  created_at: "2026-05-16T10:00:00.000Z",
  updated_at: "2026-05-16T11:00:00.000Z",
};

const NOW = new Date("2026-05-16T12:00:00.000Z");

describe("FsNowMdWriter", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-now-md-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("writes the injected format function output to .maestro/tasks/NOW.md", async () => {
    const writer = new FsNowMdWriter({
      repoRoot: root,
      format: (tasks, now) =>
        `STUB ${tasks.length} ${now.toISOString()}\n`,
    });
    await writer.write([TASK], NOW);
    const content = await readFile(
      join(root, ".maestro/tasks/NOW.md"),
      "utf8",
    );
    expect(content).toBe(`STUB 1 ${NOW.toISOString()}\n`);
  });

  it("creates .maestro/tasks/ if it does not exist", async () => {
    const writer = new FsNowMdWriter({
      repoRoot: root,
      format: () => "ok\n",
    });
    await writer.write([], NOW);
    const dir = await stat(join(root, ".maestro/tasks"));
    expect(dir.isDirectory()).toBe(true);
  });

  it("uses new Date() when no now arg is passed (smoke)", async () => {
    let captured: Date | undefined;
    const writer = new FsNowMdWriter({
      repoRoot: root,
      format: (_tasks, now) => {
        captured = now;
        return "ok\n";
      },
    });
    const before = Date.now();
    await writer.write([]);
    const after = Date.now();
    expect(captured).toBeDefined();
    const ts = captured!.getTime();
    expect(ts).toBeGreaterThanOrEqual(before);
    expect(ts).toBeLessThanOrEqual(after);
  });

  it("overwrites existing NOW.md on each call", async () => {
    let n = 0;
    const writer = new FsNowMdWriter({
      repoRoot: root,
      format: () => `iter ${++n}\n`,
    });
    await writer.write([], NOW);
    await writer.write([], NOW);
    const content = await readFile(
      join(root, ".maestro/tasks/NOW.md"),
      "utf8",
    );
    expect(content).toBe("iter 2\n");
  });
});
