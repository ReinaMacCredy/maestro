import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  FsTaskContinuationHistoryStoreAdapter,
  type TaskContinuationEvent,
} from "@/features/task/index.js";

describe("FsTaskContinuationHistoryStoreAdapter", () => {
  let tmpDir: string;
  let store: FsTaskContinuationHistoryStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-continuation-history-"));
    store = new FsTaskContinuationHistoryStoreAdapter(tmpDir);
  });

  function makeEvent(
    kind: TaskContinuationEvent["kind"],
    at: string,
    summary: string,
  ): TaskContinuationEvent {
    switch (kind) {
      case "snapshot":
        return { kind, at, summary, currentState: summary };
      case "decision":
        return { kind, at, summary, decision: summary, active: true };
      case "next_action_set":
        return { kind, at, summary, nextAction: summary };
      case "blocker_set":
        return { kind, at, summary, blockerTaskIds: ["tsk-blocker"] };
      case "handoff_created":
        return { kind, at, summary, handoffId: "2026-04-21-001", agent: "codex", sessionId: "codex-1" };
      case "handoff_picked_up":
        return { kind, at, summary, handoffId: "2026-04-21-001", agent: "claude", sessionId: "claude-1" };
      case "agent_takeover":
        return {
          kind,
          at,
          summary,
          reason: "resume",
          to: { type: "claude", sessionId: "claude-1", lastSeenAt: at },
          from: { type: "codex", sessionId: "codex-1", lastSeenAt: "2026-04-21T09:59:00.000Z" },
        };
      case "task_completed":
        return { kind, at, summary, reason: "done" };
      case "task_reopened":
        return { kind, at, summary, reason: "retry" };
    }
  }

  it("returns no events when local history does not exist", async () => {
    expect(await store.listRecent("tsk-123", 5)).toEqual([]);
  });

  it("appends events and returns the latest entries in chronological order", async () => {
    await store.append("tsk-123", makeEvent("snapshot", "2026-04-21T09:00:00.000Z", "first"));
    await store.append("tsk-123", makeEvent("decision", "2026-04-21T09:05:00.000Z", "second"));
    await store.append("tsk-123", makeEvent("next_action_set", "2026-04-21T09:10:00.000Z", "third"));

    const events = await store.listRecent("tsk-123", 2);
    expect(events.map((event) => event.summary)).toEqual(["second", "third"]);
  });

  it("skips malformed local history lines instead of failing the whole read", async () => {
    const historyDir = join(tmpDir, ".maestro", "tasks", "local-history");
    await mkdir(historyDir, { recursive: true });
    const historyPath = join(historyDir, "tsk-123.jsonl");

    await Bun.write(
      historyPath,
      [
        JSON.stringify(makeEvent("snapshot", "2026-04-21T09:00:00.000Z", "good")),
        "{bad json",
        JSON.stringify(makeEvent("task_completed", "2026-04-21T09:10:00.000Z", "done")),
      ].join("\n") + "\n",
    );

    const events = await store.listRecent("tsk-123", 5);
    expect(events.map((event) => event.summary)).toEqual(["good", "done"]);
  });
});
