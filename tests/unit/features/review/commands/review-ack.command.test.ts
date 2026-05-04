import { describe, expect, it } from "bun:test";
import { Command } from "commander";
import { registerReviewCommand } from "@/features/review/index.js";
import { mockEvidenceStore } from "../../../../helpers/mocks.js";
import type { RecordEvidenceInput } from "@/features/evidence/index.js";
import type { EvidenceRow } from "@/features/evidence/index.js";
import type { ReviewAckPayload } from "@/features/evidence/index.js";
import type { EvidenceStorePort } from "@/features/evidence/index.js";

function makeProgram(
  store: EvidenceStorePort,
  username = "testuser",
): Command {
  const program = new Command().exitOverride();
  registerReviewCommand(program, {
    getServices: () => ({ evidenceStore: store }),
    recordEvidence: async (
      s: EvidenceStorePort,
      input: RecordEvidenceInput,
    ): Promise<EvidenceRow> => {
      const row: EvidenceRow = {
        schema_version: 3,
        id: "evd-test01",
        task_id: input.task_id,
        kind: input.kind,
        witness_level: input.witness_level,
        created_at: "2026-05-05T08:00:00.000Z",
        payload: input.payload,
      };
      await s.append(row);
      return row;
    },
    getUsername: () => username,
  });
  return program;
}

describe("review ack command", () => {
  it("writes a review-ack Evidence row with correct kind and payload", async () => {
    const store = mockEvidenceStore();
    const program = makeProgram(store, "alice");

    await program.parseAsync([
      "node", "maestro",
      "review", "ack",
      "--task", "tsk-aaaaaa",
      "--verdict", "vrd-bbbbbb",
      "--criterion", "All tests pass",
      "--criterion", "No critical findings",
    ]);

    const rows = await store.list({ task_id: "tsk-aaaaaa" });
    expect(rows).toHaveLength(1);

    const row = rows[0]!;
    expect(row.kind).toBe("review-ack");
    expect(row.task_id).toBe("tsk-aaaaaa");
    expect(row.witness_level).toBe("agent-claimed-locally");

    const payload = row.payload as ReviewAckPayload;
    expect(payload.verdictId).toBe("vrd-bbbbbb");
    expect(payload.ackedBy).toBe("alice");
    expect(payload.criteria).toHaveLength(2);
    expect(payload.criteria[0]).toBe("All tests pass");
    expect(payload.criteria[1]).toBe("No critical findings");
  });

  it("rejects when --criterion is missing", async () => {
    const store = mockEvidenceStore();
    const program = makeProgram(store);

    await expect(
      program.parseAsync([
        "node", "maestro",
        "review", "ack",
        "--task", "tsk-aaaaaa",
        "--verdict", "vrd-bbbbbb",
      ]),
    ).rejects.toThrow("--criterion is required");
  });

  it("accumulates multiple --criterion flags into criteria array", async () => {
    const store = mockEvidenceStore();
    const program = makeProgram(store, "bob");

    await program.parseAsync([
      "node", "maestro",
      "review", "ack",
      "--task", "tsk-cccccc",
      "--verdict", "vrd-dddddd",
      "--criterion", "Criterion A",
      "--criterion", "Criterion B",
      "--criterion", "Criterion C",
    ]);

    const rows = await store.list({ task_id: "tsk-cccccc" });
    expect(rows).toHaveLength(1);

    const payload = rows[0]!.payload as ReviewAckPayload;
    expect(payload.criteria).toHaveLength(3);
    expect(payload.criteria).toEqual(["Criterion A", "Criterion B", "Criterion C"]);
    expect(payload.ackedBy).toBe("bob");
  });
});
