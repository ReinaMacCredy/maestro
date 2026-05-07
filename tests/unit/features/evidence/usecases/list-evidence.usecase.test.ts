import { describe, expect, it } from "bun:test";
import { listEvidence } from "@/features/evidence/usecases/list-evidence.usecase.js";
import { mockEvidenceStore } from "../../../../helpers/mocks.js";
import type { EvidenceRow } from "@/features/evidence/domain/types.js";

function commandRow(overrides: Partial<EvidenceRow<"command">> = {}): EvidenceRow<"command"> {
  return {
    schema_version: 1,
    id: overrides.id ?? "evd-1700000000000-aaaaaa",
    task_id: overrides.task_id ?? "tsk-aaaaaa",
    session_id: overrides.session_id,
    kind: "command",
    witness_level: overrides.witness_level ?? "witnessed-by-maestro",
    created_at: overrides.created_at ?? "2026-05-03T10:00:00.000Z",
    payload: overrides.payload ?? { command: "bun test", exit: 0 },
  };
}

function noteRow(overrides: Partial<EvidenceRow<"manual-note">> = {}): EvidenceRow<"manual-note"> {
  return {
    schema_version: 1,
    id: overrides.id ?? "evd-1700000000001-bbbbbb",
    task_id: overrides.task_id ?? "tsk-aaaaaa",
    session_id: overrides.session_id,
    kind: "manual-note",
    witness_level: overrides.witness_level ?? "agent-claimed-locally",
    created_at: overrides.created_at ?? "2026-05-03T11:00:00.000Z",
    payload: overrides.payload ?? { note: "ok" },
  };
}

describe("listEvidence", () => {
  it("returns an empty list for an empty store", async () => {
    expect(await listEvidence(mockEvidenceStore())).toEqual([]);
  });

  it("returns all rows sorted by created_at ascending when no filter is supplied", async () => {
    const a = commandRow({ id: "evd-1700000000003-aaaaaa", created_at: "2026-05-03T12:00:00.000Z" });
    const b = commandRow({ id: "evd-1700000000002-bbbbbb", created_at: "2026-05-03T08:00:00.000Z" });
    const c = noteRow({ id: "evd-1700000000004-cccccc", created_at: "2026-05-03T10:00:00.000Z" });
    const store = mockEvidenceStore([a, b, c]);

    const result = await listEvidence(store);
    expect(result.map((r) => r.id)).toEqual([b.id, c.id, a.id]);
  });

  it("filters by task_id", async () => {
    const match = commandRow({ id: "evd-1700000000010-aaaaaa", task_id: "tsk-aaaaaa" });
    const skip = commandRow({ id: "evd-1700000000011-bbbbbb", task_id: "tsk-bbbbbb" });
    const store = mockEvidenceStore([match, skip]);

    const result = await listEvidence(store, { task_id: "tsk-aaaaaa" });
    expect(result.map((r) => r.id)).toEqual([match.id]);
  });

  it("filters by session_id across tasks", async () => {
    const matchA = commandRow({ id: "evd-1700000000020-aaaaaa", task_id: "tsk-aaaaaa", session_id: "sess-1" });
    const matchB = noteRow({ id: "evd-1700000000021-bbbbbb", task_id: "tsk-bbbbbb", session_id: "sess-1" });
    const skip = commandRow({ id: "evd-1700000000022-cccccc", task_id: "tsk-cccccc", session_id: "sess-2" });
    const store = mockEvidenceStore([matchA, matchB, skip]);

    const result = await listEvidence(store, { session_id: "sess-1" });
    const ids = result.map((r) => r.id).sort();
    expect(ids).toEqual([matchA.id, matchB.id].sort());
  });

  it("filters by kind across tasks", async () => {
    const cmd = commandRow({ id: "evd-1700000000030-aaaaaa", task_id: "tsk-aaaaaa" });
    const note = noteRow({ id: "evd-1700000000031-bbbbbb", task_id: "tsk-bbbbbb" });
    const store = mockEvidenceStore([cmd, note]);

    expect((await listEvidence(store, { kind: "command" })).map((r) => r.id)).toEqual([cmd.id]);
    expect((await listEvidence(store, { kind: "manual-note" })).map((r) => r.id)).toEqual([note.id]);
  });

  it("intersects task_id and kind filters", async () => {
    const target = commandRow({ id: "evd-1700000000040-aaaaaa", task_id: "tsk-aaaaaa" });
    const wrongKind = noteRow({ id: "evd-1700000000041-bbbbbb", task_id: "tsk-aaaaaa" });
    const wrongTask = commandRow({ id: "evd-1700000000042-cccccc", task_id: "tsk-bbbbbb" });
    const store = mockEvidenceStore([target, wrongKind, wrongTask]);

    const result = await listEvidence(store, { task_id: "tsk-aaaaaa", kind: "command" });
    expect(result.map((r) => r.id)).toEqual([target.id]);
  });
});
