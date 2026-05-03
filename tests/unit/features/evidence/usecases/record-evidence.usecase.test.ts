import { describe, expect, it } from "bun:test";
import { recordEvidence } from "@/features/evidence/usecases/record-evidence.usecase.js";
import { EVIDENCE_ID_PATTERN } from "@/features/evidence/domain/evidence-id.js";
import { mockEvidenceStore } from "../../../../helpers/mocks.js";

const ISO_TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/;

describe("recordEvidence", () => {
  it("returns a row with schema_version 2, generated id and ISO created_at, and preserves the input fields", async () => {
    const store = mockEvidenceStore();

    const row = await recordEvidence(store, {
      task_id: "tsk-aaaaaa",
      session_id: "sess-1",
      kind: "command",
      payload: { command: "bun test", exit: 0, duration_ms: 1234 },
      witness_level: "witnessed-by-maestro",
    });

    expect(row.schema_version).toBe(2);
    expect(row.id).toMatch(EVIDENCE_ID_PATTERN);
    expect(row.created_at).toMatch(ISO_TIMESTAMP);
    expect(row.task_id).toBe("tsk-aaaaaa");
    expect(row.session_id).toBe("sess-1");
    expect(row.kind).toBe("command");
    expect(row.payload).toEqual({ command: "bun test", exit: 0, duration_ms: 1234 });
    expect(row.witness_level).toBe("witnessed-by-maestro");
  });

  it("persists the row through the storage port", async () => {
    const store = mockEvidenceStore();

    const row = await recordEvidence(store, {
      task_id: "tsk-bbbbbb",
      kind: "command",
      payload: { command: "bun run build", exit: 0 },
      witness_level: "witnessed-by-ci",
    });

    const list = await store.list();
    expect(list).toEqual([row]);
    expect(await store.read(row.id)).toEqual(row);
  });

  it("mints distinct ids for sequential calls", async () => {
    const store = mockEvidenceStore();

    const a = await recordEvidence(store, {
      task_id: "tsk-aaaaaa",
      kind: "command",
      payload: { command: "step a", exit: 0 },
      witness_level: "witnessed-by-maestro",
    });
    const b = await recordEvidence(store, {
      task_id: "tsk-aaaaaa",
      kind: "command",
      payload: { command: "step b", exit: 0 },
      witness_level: "witnessed-by-maestro",
    });

    expect(a.id).not.toBe(b.id);
  });

  it("round-trips a manual-note kind with its payload", async () => {
    const store = mockEvidenceStore();

    const row = await recordEvidence(store, {
      task_id: "tsk-cccccc",
      kind: "manual-note",
      payload: { note: "manually verified UI on staging", criterion_id: "ui-01" },
      witness_level: "agent-claimed-and-not-reproducible",
    });

    expect(row.kind).toBe("manual-note");
    expect(row.payload).toEqual({
      note: "manually verified UI on staging",
      criterion_id: "ui-01",
    });
  });

  it("omits session_id from the persisted row when not provided", async () => {
    const store = mockEvidenceStore();

    const row = await recordEvidence(store, {
      task_id: "tsk-dddddd",
      kind: "command",
      payload: { command: "no session", exit: 0 },
      witness_level: "agent-claimed-locally",
    });

    expect(row.session_id).toBeUndefined();
  });
});
