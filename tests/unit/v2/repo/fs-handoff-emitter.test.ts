import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsHandoffEmitter } from "@/v2/repo/fs-handoff-emitter.adapter.js";

describe("FsHandoffEmitter", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-handoff-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("writes one JSON file per envelope under .maestro/handoffs/", async () => {
    const emitter = new FsHandoffEmitter({ repoRoot: root });
    await emitter.emit({
      id: "hnd-1",
      task_id: "tsk-1",
      trigger_verb: "task:claim",
      created_at: "2026-05-15T10:00:00.000Z",
      agent_id: "agent-a",
    });
    const raw = await readFile(
      join(root, ".maestro/handoffs/hnd-1.json"),
      "utf8",
    );
    const parsed = JSON.parse(raw) as { id: string; trigger_verb: string };
    expect(parsed.id).toBe("hnd-1");
    expect(parsed.trigger_verb).toBe("task:claim");
  });

  it("list() returns empty when the directory does not exist", async () => {
    const emitter = new FsHandoffEmitter({ repoRoot: root });
    expect(await emitter.list()).toEqual([]);
  });

  it("list() returns every emitted envelope", async () => {
    const emitter = new FsHandoffEmitter({ repoRoot: root });
    await emitter.emit({
      id: "hnd-a",
      task_id: "tsk-1",
      trigger_verb: "task:claim",
      created_at: "2026-05-15T10:00:00.000Z",
    });
    await emitter.emit({
      id: "hnd-b",
      task_id: "tsk-2",
      trigger_verb: "task:block",
      created_at: "2026-05-15T10:01:00.000Z",
      reason: "missing-credentials",
    });
    const list = await emitter.list();
    expect(list.length).toBe(2);
    expect(list.map((r) => r.id).sort()).toEqual(["hnd-a", "hnd-b"]);
  });

  it("get() round-trips a single envelope", async () => {
    const emitter = new FsHandoffEmitter({ repoRoot: root });
    await emitter.emit({
      id: "hnd-x",
      task_id: "tsk-x",
      trigger_verb: "task:abandon",
      created_at: "2026-05-15T10:02:00.000Z",
      reason: "out-of-scope",
    });
    const got = await emitter.get("hnd-x");
    expect(got?.trigger_verb).toBe("task:abandon");
    expect(got?.reason).toBe("out-of-scope");
  });

  it("get() returns undefined for unknown ids", async () => {
    const emitter = new FsHandoffEmitter({ repoRoot: root });
    expect(await emitter.get("hnd-missing")).toBeUndefined();
  });
});
