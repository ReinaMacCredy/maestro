import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsRunStateStoreAdapter } from "@/features/task/adapters/fs-run-state-store.adapter.js";
import type { RunState } from "@/features/task/domain/run-state.js";

function makeRunState(overrides: Partial<RunState> = {}): RunState {
  return {
    schemaVersion: 1,
    taskId: "tsk-aaaaaa",
    retryCount: 2,
    wallClockElapsedSeconds: 120,
    lastUpdatedAt: "2026-01-01T00:00:00.000Z",
    ...overrides,
  };
}

describe("FsRunStateStoreAdapter", () => {
  let tmpDir: string;
  let store: FsRunStateStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-run-state-test-"));
    store = new FsRunStateStoreAdapter(tmpDir);
  });

  it("read of missing file returns undefined", async () => {
    const result = await store.read("tsk-aaaaaa");
    expect(result).toBeUndefined();
  });

  it("round-trip: write then read returns the same state", async () => {
    const state = makeRunState();
    await store.write("tsk-aaaaaa", state);
    const loaded = await store.read("tsk-aaaaaa");
    expect(loaded).toBeDefined();
    expect(loaded?.retryCount).toBe(2);
    expect(loaded?.wallClockElapsedSeconds).toBe(120);
    expect(loaded?.taskId).toBe("tsk-aaaaaa");
  });

  it("write then read preserves optional tokensUsed field", async () => {
    const state = makeRunState({ tokensUsed: 5000 });
    await store.write("tsk-aaaaaa", state);
    const loaded = await store.read("tsk-aaaaaa");
    expect(loaded?.tokensUsed).toBe(5000);
  });

  it("increment from missing initializes to zeros and applies delta", async () => {
    const result = await store.increment("tsk-aaaaaa", { retryCount: 1 });
    expect(result.retryCount).toBe(1);
    expect(result.wallClockElapsedSeconds).toBe(0);
    expect(result.taskId).toBe("tsk-aaaaaa");
    expect(result.schemaVersion).toBe(1);
  });

  it("increment from existing state adds delta values", async () => {
    const initial = makeRunState({ retryCount: 3, wallClockElapsedSeconds: 200 });
    await store.write("tsk-aaaaaa", initial);

    const result = await store.increment("tsk-aaaaaa", { retryCount: 1, wallClockElapsedSeconds: 50 });
    expect(result.retryCount).toBe(4);
    expect(result.wallClockElapsedSeconds).toBe(250);
  });

  it("increment persists the new state so a subsequent read sees it", async () => {
    await store.increment("tsk-aaaaaa", { retryCount: 1 });
    await store.increment("tsk-aaaaaa", { retryCount: 1 });
    const loaded = await store.read("tsk-aaaaaa");
    expect(loaded?.retryCount).toBe(2);
  });

  it("increment with tokensUsed delta accumulates tokens", async () => {
    await store.increment("tsk-aaaaaa", { tokensUsed: 1000 });
    const result = await store.increment("tsk-aaaaaa", { tokensUsed: 500 });
    expect(result.tokensUsed).toBe(1500);
  });

  it("increment without tokensUsed delta leaves tokensUsed undefined when not previously set", async () => {
    const result = await store.increment("tsk-aaaaaa", { retryCount: 1 });
    expect(result.tokensUsed).toBeUndefined();
  });

  it("rejects invalid task IDs", async () => {
    await expect(store.read("invalid-id")).rejects.toThrow();
  });

  it("isolates state per task id", async () => {
    await store.write("tsk-aaaaaa", makeRunState({ taskId: "tsk-aaaaaa", retryCount: 5 }));
    await store.write("tsk-bbbbbb", makeRunState({ taskId: "tsk-bbbbbb", retryCount: 1 }));

    const a = await store.read("tsk-aaaaaa");
    const b = await store.read("tsk-bbbbbb");
    expect(a?.retryCount).toBe(5);
    expect(b?.retryCount).toBe(1);
  });
});
