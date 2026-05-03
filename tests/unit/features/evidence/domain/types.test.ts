import { describe, expect, it } from "bun:test";
import type {
  EvidenceRow,
  WitnessLevel,
} from "@/features/evidence/domain/types.js";

describe("evidence domain types", () => {
  it("instantiates a command-kind row", () => {
    const row: EvidenceRow<"command"> = {
      schema_version: 1,
      id: "evd-000001",
      task_id: "tsk-a1b2c3",
      session_id: "ses-abcdef",
      kind: "command",
      witness_level: "witnessed-by-maestro",
      created_at: "2026-05-03T12:00:00.000Z",
      payload: {
        command: "bun test",
        exit: 0,
        log_path: ".maestro/logs/evd-000001.log",
        duration_ms: 1234,
        criterion_id: "crt-001",
      },
    };

    expect(row.schema_version).toBe(1);
    expect(row.kind).toBe("command");
    expect(row.payload.command).toBe("bun test");
    expect(row.payload.exit).toBe(0);
    expect(row.payload.log_path).toBe(".maestro/logs/evd-000001.log");
    expect(row.payload.duration_ms).toBe(1234);
    expect(row.payload.criterion_id).toBe("crt-001");
    expect(row.session_id).toBe("ses-abcdef");
    expect(row.witness_level).toBe("witnessed-by-maestro");
    expect(row.created_at).toBe("2026-05-03T12:00:00.000Z");
    expect(row.task_id).toBe("tsk-a1b2c3");
    expect(row.id).toBe("evd-000001");
  });

  it("instantiates a manual-note-kind row", () => {
    const row: EvidenceRow<"manual-note"> = {
      schema_version: 1,
      id: "evd-000002",
      task_id: "tsk-a1b2c3",
      kind: "manual-note",
      witness_level: "agent-claimed-locally",
      created_at: "2026-05-03T12:05:00.000Z",
      payload: {
        note: "verified by hand in browser",
        criterion_id: "crt-002",
      },
    };

    expect(row.kind).toBe("manual-note");
    expect(row.payload.note).toBe("verified by hand in browser");
    expect(row.payload.criterion_id).toBe("crt-002");
    expect(row.session_id).toBeUndefined();
  });

  it("accepts each WitnessLevel", () => {
    const levels = [
      "witnessed-by-maestro",
      "witnessed-by-ci",
      "agent-claimed-locally",
      "agent-claimed-and-not-reproducible",
    ] satisfies WitnessLevel[];

    expect(levels).toHaveLength(4);
  });
});
