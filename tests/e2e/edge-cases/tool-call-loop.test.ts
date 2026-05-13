/**
 * Edge Case 2 (tool-call loop detection): when the same evidence kind +
 * payload hash repeats >=3 times in a run with no verdict-requested or
 * session boundary between them, `task introspect` must surface a loopWarning
 * so the agent breaks the cycle.
 */
import { describe, it, expect } from "bun:test";
import {
  composeTaskIntrospection,
  formatTaskIntrospectionMarkdown,
} from "@/features/task/usecases/compose-task-introspection.usecase.js";
import type { TaskIntrospectionDeps } from "@/features/task/usecases/compose-task-introspection.usecase.js";
import type { EvidenceRow } from "@/features/evidence/index.js";
import type { Task } from "@/features/task/index.js";

function makeTask(): Task {
  return {
    id: "task-loop-001",
    title: "Loop test",
    type: "implementation",
    priority: 2,
    status: "in-progress",
    labels: [],
    blocks: [],
    blockedBy: [],
    createdAt: "2026-05-01T00:00:00.000Z",
    updatedAt: "2026-05-01T00:00:00.000Z",
  };
}

function makeCmdRow(id: string, ts: string, command: string, exit: number): EvidenceRow<"command"> {
  return {
    schema_version: 3,
    id,
    task_id: "task-loop-001",
    kind: "command",
    witness_level: "witnessed-by-maestro",
    created_at: ts,
    payload: { command, exit },
  };
}

function makeDeps(rows: readonly EvidenceRow[]): TaskIntrospectionDeps {
  const task = makeTask();
  return {
    taskStore: { get: async () => task, all: async () => [task] } as any,
    continuationStore: {
      getActive: async () => undefined,
      getCompleted: async () => undefined,
    } as any,
    continuationHistory: { listRecent: async () => [] } as any,
    specStore: { read: async () => undefined } as any,
    verdictStore: { readLatest: async () => undefined } as any,
    evidenceStore: { list: async () => rows } as any,
    runStateStore: { read: async () => undefined } as any,
    contractStore: { getByTaskId: async () => undefined } as any,
    contractVersionStore: {
      readCurrent: async () => undefined,
      listVersions: async () => [],
      readAtVersion: async () => undefined,
    } as any,
    repoRoot: "/tmp/repo-fake",
    checkCommitReachable: async () => true,
  };
}

describe("Edge Case 2: tool-call loop detection", () => {
  it("emits a loopWarning when the same (kind, payload) repeats 3 times in a row", async () => {
    const rows = [
      makeCmdRow("ev-1", "2026-05-01T00:00:01.000Z", "bun test", 1),
      makeCmdRow("ev-2", "2026-05-01T00:00:02.000Z", "bun test", 1),
      makeCmdRow("ev-3", "2026-05-01T00:00:03.000Z", "bun test", 1),
    ];
    const view = await composeTaskIntrospection(makeDeps(rows), "task-loop-001");
    expect(view.loopWarning).toBeDefined();
    expect(view.loopWarning?.kind).toBe("command");
    expect(view.loopWarning?.count).toBe(3);
  });

  it("does not emit a loopWarning when count < 3", async () => {
    const rows = [
      makeCmdRow("ev-1", "2026-05-01T00:00:01.000Z", "bun test", 1),
      makeCmdRow("ev-2", "2026-05-01T00:00:02.000Z", "bun test", 1),
    ];
    const view = await composeTaskIntrospection(makeDeps(rows), "task-loop-001");
    expect(view.loopWarning).toBeUndefined();
  });

  it("resets the run when a verdict-requested row appears between identical commands", async () => {
    const rows: EvidenceRow[] = [
      makeCmdRow("ev-1", "2026-05-01T00:00:01.000Z", "bun test", 1),
      makeCmdRow("ev-2", "2026-05-01T00:00:02.000Z", "bun test", 1),
      {
        schema_version: 3,
        id: "ev-vr",
        task_id: "task-loop-001",
        kind: "verdict-requested",
        witness_level: "witnessed-by-maestro",
        created_at: "2026-05-01T00:00:02.500Z",
        payload: {} as any,
      } as any,
      makeCmdRow("ev-3", "2026-05-01T00:00:03.000Z", "bun test", 1),
    ];
    const view = await composeTaskIntrospection(makeDeps(rows), "task-loop-001");
    expect(view.loopWarning).toBeUndefined();
  });

  it("markdown surfaces recovery hint when looped", async () => {
    const rows = [
      makeCmdRow("ev-1", "2026-05-01T00:00:01.000Z", "bun test", 1),
      makeCmdRow("ev-2", "2026-05-01T00:00:02.000Z", "bun test", 1),
      makeCmdRow("ev-3", "2026-05-01T00:00:03.000Z", "bun test", 1),
    ];
    const view = await composeTaskIntrospection(makeDeps(rows), "task-loop-001");
    const md = formatTaskIntrospectionMarkdown(view);
    expect(md).toContain("Loop warning");
    expect(md).toContain("maestro ralph review");
  });
});
