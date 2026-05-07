import { describe, expect, it } from "bun:test";
import { buildTaskBoard } from "@/tui/state/task-board.js";
import { mockEvidenceStore, mockTaskStore } from "../../../helpers/mocks.js";
import type { Task } from "@/features/task";
import type { EvidenceRow } from "@/features/evidence";

function makeTask(id: string): Task {
  return {
    id,
    title: `Task ${id}`,
    type: "task",
    priority: 1,
    status: "pending",
    labels: [],
    blocks: [],
    blockedBy: [],
    createdAt: "2026-05-03T00:00:00.000Z",
    updatedAt: "2026-05-03T00:00:00.000Z",
  };
}

function makeEvidenceRow(
  overrides: Partial<EvidenceRow> & { id: string; task_id: string },
): EvidenceRow {
  return {
    schema_version: 1,
    kind: "command",
    witness_level: "agent-claimed-locally",
    created_at: "2026-05-03T00:00:00.000Z",
    payload: { command: "bun test", exit: 0 },
    ...overrides,
  } as EvidenceRow;
}

describe("buildTaskBoard — evidence integration", () => {
  it("returns evidenceCount=0 and empty recentEvidence when evidenceStore is undefined", async () => {
    const store = mockTaskStore([makeTask("tsk-aaaaaa")]);
    const board = await buildTaskBoard(store);
    expect(board).not.toBeNull();
    const item = board!.columns.pending[0]!;
    expect(item.evidenceCount).toBe(0);
    expect(item.recentEvidence).toEqual([]);
  });

  it("returns evidenceCount=0 and empty recentEvidence when store has no rows for task", async () => {
    const store = mockTaskStore([makeTask("tsk-aaaaaa")]);
    const evidenceStore = mockEvidenceStore([]);
    const board = await buildTaskBoard(store, evidenceStore);
    expect(board).not.toBeNull();
    const item = board!.columns.pending[0]!;
    expect(item.evidenceCount).toBe(0);
    expect(item.recentEvidence).toEqual([]);
  });

  it("returns correct evidenceCount and most-recent-first recentEvidence for populated store", async () => {
    const task = makeTask("tsk-aaaaaa");
    const rows = [
      makeEvidenceRow({ id: "evd-0000000000001-aaaaaa", task_id: "tsk-aaaaaa", created_at: "2026-05-03T00:00:01.000Z" }),
      makeEvidenceRow({ id: "evd-0000000000002-aaaaaa", task_id: "tsk-aaaaaa", created_at: "2026-05-03T00:00:02.000Z" }),
      makeEvidenceRow({ id: "evd-0000000000003-aaaaaa", task_id: "tsk-aaaaaa", created_at: "2026-05-03T00:00:03.000Z" }),
    ];
    const store = mockTaskStore([task]);
    const evidenceStore = mockEvidenceStore(rows);
    const board = await buildTaskBoard(store, evidenceStore);
    expect(board).not.toBeNull();
    const item = board!.columns.pending[0]!;
    expect(item.evidenceCount).toBe(3);
    expect(item.recentEvidence).toHaveLength(3);
    // most-recent-first
    expect(item.recentEvidence[0]!.id).toBe("evd-0000000000003-aaaaaa");
    expect(item.recentEvidence[1]!.id).toBe("evd-0000000000002-aaaaaa");
    expect(item.recentEvidence[2]!.id).toBe("evd-0000000000001-aaaaaa");
  });

  it("caps recentEvidence at 5 items but reports full evidenceCount", async () => {
    const task = makeTask("tsk-aaaaaa");
    const rows = Array.from({ length: 8 }, (_, i) => makeEvidenceRow({
      id: `evd-000000000000${i + 1}-aaaaaa`,
      task_id: "tsk-aaaaaa",
      created_at: `2026-05-03T00:00:0${i + 1}.000Z`,
    }));
    const store = mockTaskStore([task]);
    const evidenceStore = mockEvidenceStore(rows);
    const board = await buildTaskBoard(store, evidenceStore);
    expect(board).not.toBeNull();
    const item = board!.columns.pending[0]!;
    expect(item.evidenceCount).toBe(8);
    expect(item.recentEvidence).toHaveLength(5);
    // first item in recentEvidence should be the most recent (index 7 = created_at 08)
    expect(item.recentEvidence[0]!.id).toBe("evd-0000000000008-aaaaaa");
  });

  it("does not include evidence from other tasks", async () => {
    const tasks = [makeTask("tsk-aaaaaa"), makeTask("tsk-bbbbbb")];
    const rows = [
      makeEvidenceRow({ id: "evd-0000000000001-aaaaaa", task_id: "tsk-aaaaaa" }),
      makeEvidenceRow({ id: "evd-0000000000002-aaaaaa", task_id: "tsk-bbbbbb" }),
    ];
    const store = mockTaskStore(tasks);
    const evidenceStore = mockEvidenceStore(rows);
    const board = await buildTaskBoard(store, evidenceStore);
    expect(board).not.toBeNull();
    const itemA = board!.columns.pending.find((item) => item.id === "tsk-aaaaaa")!;
    const itemB = board!.columns.pending.find((item) => item.id === "tsk-bbbbbb")!;
    expect(itemA.evidenceCount).toBe(1);
    expect(itemB.evidenceCount).toBe(1);
    expect(itemA.recentEvidence[0]!.id).toBe("evd-0000000000001-aaaaaa");
    expect(itemB.recentEvidence[0]!.id).toBe("evd-0000000000002-aaaaaa");
  });

  it("summary omits payload field", async () => {
    const task = makeTask("tsk-aaaaaa");
    const row = makeEvidenceRow({ id: "evd-0000000000001-aaaaaa", task_id: "tsk-aaaaaa" });
    const store = mockTaskStore([task]);
    const evidenceStore = mockEvidenceStore([row]);
    const board = await buildTaskBoard(store, evidenceStore);
    const summary = board!.columns.pending[0]!.recentEvidence[0]!;
    expect("payload" in summary).toBe(false);
    expect(summary.id).toBe("evd-0000000000001-aaaaaa");
  });
});
