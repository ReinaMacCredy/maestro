import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { access, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { CompositeLaunchStore, FsLaunchStoreAdapter } from "@/features/handoff";

describe("CompositeLaunchStore", () => {
  let localDir: string;
  let globalDir: string;
  let local: FsLaunchStoreAdapter;
  let global: FsLaunchStoreAdapter;
  let store: CompositeLaunchStore;

  beforeEach(async () => {
    localDir = await mkdtemp(join(tmpdir(), "maestro-composite-local-"));
    globalDir = await mkdtemp(join(tmpdir(), "maestro-composite-global-"));
    local = new FsLaunchStoreAdapter(localDir);
    global = new FsLaunchStoreAdapter(globalDir);
    store = new CompositeLaunchStore(local, global);
  });

  afterEach(async () => {
    await rm(localDir, { recursive: true, force: true });
    await rm(globalDir, { recursive: true, force: true });
  });

  async function createPacket(opts: {
    readonly taskId?: string;
    readonly via: "composite" | "local" | "global";
  }) {
    const input = {
      task: "task",
      name: "name",
      agent: "claude" as const,
      model: "opus",
      wait: false,
      sourceDir: "/tmp/irrelevant",
      targetDir: "/tmp/irrelevant",
      refs: opts.taskId ? { taskId: opts.taskId } : {},
      prompt: "## Task\n\ntask\n",
    };
    const target = opts.via === "local" ? local : opts.via === "global" ? global : store;
    return target.create(input);
  }

  it("routes create(task-less) to the global store", async () => {
    const record = await createPacket({ via: "composite" });

    await access(join(globalDir, ".maestro", "launches", record.id, "launch.json"));
    await expect(access(join(localDir, ".maestro", "launches", record.id, "launch.json"))).rejects.toThrow();
  });

  it("routes create(task-linked) to the local store", async () => {
    const record = await createPacket({ via: "composite", taskId: "tsk-123" });

    await access(join(localDir, ".maestro", "launches", record.id, "launch.json"));
    await expect(access(join(globalDir, ".maestro", "launches", record.id, "launch.json"))).rejects.toThrow();
  });

  it("get() returns the local record when both stores have a colliding id", async () => {
    const localRecord = await createPacket({ via: "local", taskId: "tsk-local" });
    const globalRecord = await createPacket({ via: "global" });
    expect(localRecord.id).toBe(globalRecord.id);

    const fromComposite = await store.get(localRecord.id);
    expect(fromComposite?.id).toBe(localRecord.id);
    expect(fromComposite?.refs.taskId).toBe("tsk-local");
  });

  it("list() merges both stores and dedupes on id with local-first priority", async () => {
    const localRecord = await createPacket({ via: "local", taskId: "tsk-a" });
    const globalRecord = await createPacket({ via: "global" });
    expect(localRecord.id).toBe(globalRecord.id);

    const merged = await store.list();
    expect(merged).toHaveLength(1);
    expect(merged[0]!.refs.taskId).toBe("tsk-a");
  });

  it("consume() routes to the store that holds the id", async () => {
    const taskLess = await createPacket({ via: "composite" });

    const consumed = await store.consume({
      id: taskLess.id,
      agent: "claude",
      pickedUpAt: new Date().toISOString(),
    });
    expect(consumed.consumedAt).toBeTruthy();

    const reloaded = await global.get(taskLess.id);
    expect(reloaded?.consumedAt).toBeTruthy();
  });

  it("update() routes by refs.taskId", async () => {
    const taskLess = await createPacket({ via: "composite" });
    const updated = await store.update({ ...taskLess, status: "completed" });
    expect(updated.status).toBe("completed");

    const reloadedGlobal = await global.get(taskLess.id);
    expect(reloadedGlobal?.status).toBe("completed");
    const reloadedLocal = await local.get(taskLess.id);
    expect(reloadedLocal).toBeUndefined();
  });

  it("resolveArtifactPath() returns local-rooted path for task-linked refs", async () => {
    const resolved = store.resolveArtifactPath(".maestro/launches/x/output.log", { taskId: "tsk-1" });
    expect(resolved.startsWith(localDir)).toBe(true);
  });

  it("resolveArtifactPath() returns global-rooted path for task-less refs", async () => {
    const resolved = store.resolveArtifactPath(".maestro/launches/x/output.log", {});
    expect(resolved.startsWith(globalDir)).toBe(true);
  });
});
