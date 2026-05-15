import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlObservabilityAdapter } from "@/v2/repo/jsonl-observability.adapter.js";

describe("JsonlObservabilityAdapter", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-observability-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("creates the task-scoped run directory lazily on first emit", async () => {
    const adapter = new JsonlObservabilityAdapter({ repoRoot: root });
    await adapter.emit({
      task_id: "tsk-1",
      kind: "transition",
      timestamp: "2026-05-15T08:00:00.000Z",
      payload: { from_state: "draft", to_state: "claimed" },
    });
    const path = adapter.pathFor("tsk-1");
    const raw = await readFile(path, "utf8");
    const parsed = JSON.parse(raw.trim());
    expect(parsed.task_id).toBe("tsk-1");
    expect(parsed.kind).toBe("transition");
    expect(parsed.payload.from_state).toBe("draft");
  });

  it("appends one JSONL line per event in stable order", async () => {
    const adapter = new JsonlObservabilityAdapter({ repoRoot: root });
    for (let i = 0; i < 3; i++) {
      await adapter.emit({
        task_id: "tsk-seq",
        kind: "transition",
        timestamp: `2026-05-15T08:00:0${i}.000Z`,
        payload: { i },
      });
    }
    const raw = await readFile(adapter.pathFor("tsk-seq"), "utf8");
    const lines = raw.trim().split("\n");
    expect(lines.length).toBe(3);
    const events = lines.map((l) => JSON.parse(l));
    expect(events.map((e) => e.payload.i)).toEqual([0, 1, 2]);
  });

  it("isolates events per task in separate files", async () => {
    const adapter = new JsonlObservabilityAdapter({ repoRoot: root });
    await adapter.emit({
      task_id: "tsk-a",
      kind: "transition",
      timestamp: "2026-05-15T08:00:00.000Z",
      payload: {},
    });
    await adapter.emit({
      task_id: "tsk-b",
      kind: "transition",
      timestamp: "2026-05-15T08:00:01.000Z",
      payload: {},
    });
    const aRaw = await readFile(adapter.pathFor("tsk-a"), "utf8");
    const bRaw = await readFile(adapter.pathFor("tsk-b"), "utf8");
    expect(aRaw.trim().split("\n").length).toBe(1);
    expect(bRaw.trim().split("\n").length).toBe(1);
    expect(JSON.parse(aRaw).task_id).toBe("tsk-a");
    expect(JSON.parse(bRaw).task_id).toBe("tsk-b");
  });

  it("honors a custom subdir under the repoRoot", async () => {
    const adapter = new JsonlObservabilityAdapter({
      repoRoot: root,
      subdir: ".scratch/runs",
    });
    await adapter.emit({
      task_id: "tsk-sub",
      kind: "transition",
      timestamp: "2026-05-15T09:00:00.000Z",
      payload: {},
    });
    expect(adapter.pathFor("tsk-sub")).toBe(
      join(root, ".scratch/runs", "tsk-sub", "observability.jsonl"),
    );
    const raw = await readFile(adapter.pathFor("tsk-sub"), "utf8");
    expect(raw.trim().length).toBeGreaterThan(0);
  });
});
