import { describe, expect, it, mock, spyOn } from "bun:test";
import { refreshNowMd } from "@/service/refresh-now-md.js";
import type { TaskStorePort } from "@/repo/task-store.port.js";
import type { NowMdWriterPort } from "@/repo/now-md-writer.port.js";
import type { Task } from "@/types/task.js";

const TASK: Task = {
  id: "tsk-x-1",
  slug: "demo",
  title: "demo",
  state: "draft",
  blocked_by: [],
  created_at: "2026-05-16T10:00:00.000Z",
  updated_at: "2026-05-16T11:00:00.000Z",
};

function makeStore(tasks: readonly Task[]): TaskStorePort {
  return {
    create: mock(async () => {
      throw new Error("not used");
    }),
    get: mock(async () => undefined),
    update: mock(async () => {
      throw new Error("not used");
    }),
    list: mock(async () => tasks),
    listByState: mock(async () => []),
    listByMissionId: mock(async () => []),
  } as unknown as TaskStorePort;
}

describe("refreshNowMd", () => {
  it("reads tasks from the store and calls the writer with the same array", async () => {
    const writes: Array<{ tasks: readonly Task[]; now: Date | undefined }> = [];
    const writer: NowMdWriterPort = {
      write: async (tasks, now) => {
        writes.push({ tasks, now });
      },
    };
    const before = Date.now();
    await refreshNowMd({
      taskStore: makeStore([TASK]),
      nowMdWriter: writer,
    });
    const after = Date.now();
    expect(writes.length).toBe(1);
    expect(writes[0]!.tasks).toEqual([TASK]);
    expect(writes[0]!.now).toBeDefined();
    const ts = writes[0]!.now!.getTime();
    expect(ts).toBeGreaterThanOrEqual(before);
    expect(ts).toBeLessThanOrEqual(after);
  });

  it("swallows writer errors with a single console.warn", async () => {
    const warn = spyOn(console, "warn").mockImplementation(() => {});
    const writer: NowMdWriterPort = {
      write: async () => {
        throw new Error("disk full");
      },
    };
    await expect(
      refreshNowMd({
        taskStore: makeStore([]),
        nowMdWriter: writer,
      }),
    ).resolves.toBeUndefined();
    expect(warn).toHaveBeenCalledTimes(1);
    expect(warn.mock.calls[0]![0]).toContain("NOW.md refresh failed");
    expect(warn.mock.calls[0]![0]).toContain("disk full");
    warn.mockRestore();
  });

  it("swallows taskStore.list errors too", async () => {
    const warn = spyOn(console, "warn").mockImplementation(() => {});
    const broken: TaskStorePort = {
      create: mock(async () => {
        throw new Error("nope");
      }),
      get: mock(async () => undefined),
      update: mock(async () => {
        throw new Error("nope");
      }),
      list: mock(async () => {
        throw new Error("io error");
      }),
      listByState: mock(async () => []),
      listByMissionId: mock(async () => []),
    } as unknown as TaskStorePort;
    let wrote = false;
    const writer: NowMdWriterPort = {
      write: async () => {
        wrote = true;
      },
    };
    await refreshNowMd({ taskStore: broken, nowMdWriter: writer });
    expect(wrote).toBe(false);
    expect(warn).toHaveBeenCalledTimes(1);
    warn.mockRestore();
  });
});
