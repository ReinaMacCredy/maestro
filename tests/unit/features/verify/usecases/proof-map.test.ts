import { describe, expect, it } from "bun:test";
import { buildProofMap } from "@/features/verify/usecases/proof-map.js";
import type { Spec } from "@/features/spec/index.js";
import type { EvidenceRow } from "@/features/evidence/index.js";

// ─── helpers ─────────────────────────────────────────────────────────────────

function makeSpec(criteria: Array<{ id: string; text: string }>): Spec {
  return {
    schema_version: 2,
    mission_id: "msn-001",
    acceptance_criteria: criteria,
    non_goals: [],
    runtime_signals: [],
    created_at: "2026-01-01T00:00:00.000Z",
    updated_at: "2026-01-01T00:00:00.000Z",
  };
}

function makeCommandRow(overrides: {
  id: string;
  task_id: string;
  criterion_id?: string;
}): EvidenceRow {
  return {
    schema_version: 3,
    id: overrides.id,
    task_id: overrides.task_id,
    kind: "command",
    witness_level: "witnessed-by-maestro",
    created_at: "2026-01-01T00:00:00.000Z",
    payload: {
      command: "bun test",
      exit: 0,
      criterion_id: overrides.criterion_id,
    },
  };
}

function makeManualNoteRow(overrides: {
  id: string;
  task_id: string;
  criterion_id?: string;
}): EvidenceRow {
  return {
    schema_version: 3,
    id: overrides.id,
    task_id: overrides.task_id,
    kind: "manual-note",
    witness_level: "agent-claimed-locally",
    created_at: "2026-01-01T00:00:01.000Z",
    payload: {
      note: "Verified manually",
      criterion_id: overrides.criterion_id,
    },
  };
}

// ─── tests ────────────────────────────────────────────────────────────────────

describe("buildProofMap", () => {
  it("maps 3 criteria with 2 covered → entries.length === 3, uncoveredCount === 1", () => {
    const spec = makeSpec([
      { id: "c-001", text: "Feature works" },
      { id: "c-002", text: "Tests pass" },
      { id: "c-003", text: "Docs updated" },
    ]);
    const rows: EvidenceRow[] = [
      makeCommandRow({ id: "ev-001", task_id: "tsk-001", criterion_id: "c-001" }),
      makeManualNoteRow({ id: "ev-002", task_id: "tsk-001", criterion_id: "c-002" }),
    ];

    const result = buildProofMap({ taskId: "tsk-001", spec, evidenceRows: rows });

    expect(result.entries.length).toBe(3);
    expect(result.uncoveredCount).toBe(1);
    expect(result.entries[0]?.covered).toBe(true);
    expect(result.entries[1]?.covered).toBe(true);
    expect(result.entries[2]?.covered).toBe(false);
  });

  it("no spec → entries: [], uncoveredCount: 0, no error", () => {
    const result = buildProofMap({ taskId: "tsk-002", spec: undefined, evidenceRows: [] });

    expect(result.entries).toEqual([]);
    expect(result.uncoveredCount).toBe(0);
    expect(result.taskId).toBe("tsk-002");
    expect(result.missionId).toBeUndefined();
  });

  it("evidence row without criterion_id is ignored (does not affect any entry)", () => {
    const spec = makeSpec([{ id: "c-001", text: "Feature works" }]);
    const rows: EvidenceRow[] = [
      makeCommandRow({ id: "ev-001", task_id: "tsk-001" }), // no criterion_id
    ];

    const result = buildProofMap({ taskId: "tsk-001", spec, evidenceRows: rows });

    expect(result.entries[0]?.covered).toBe(false);
    expect(result.entries[0]?.evidence).toHaveLength(0);
    expect(result.uncoveredCount).toBe(1);
  });

  it("multiple evidence rows for same criterion → all listed in that entry's evidence array", () => {
    const spec = makeSpec([{ id: "c-001", text: "Feature works" }]);
    const rows: EvidenceRow[] = [
      makeCommandRow({ id: "ev-001", task_id: "tsk-001", criterion_id: "c-001" }),
      makeCommandRow({ id: "ev-002", task_id: "tsk-001", criterion_id: "c-001" }),
      makeManualNoteRow({ id: "ev-003", task_id: "tsk-001", criterion_id: "c-001" }),
    ];

    const result = buildProofMap({ taskId: "tsk-001", spec, evidenceRows: rows });

    expect(result.entries[0]?.evidence).toHaveLength(3);
    expect(result.entries[0]?.covered).toBe(true);
    expect(result.uncoveredCount).toBe(0);
  });

  it("missionId is populated from spec.mission_id", () => {
    const spec = makeSpec([{ id: "c-001", text: "works" }]);
    const result = buildProofMap({ taskId: "tsk-001", spec, evidenceRows: [] });

    expect(result.missionId).toBe("msn-001");
  });
});
