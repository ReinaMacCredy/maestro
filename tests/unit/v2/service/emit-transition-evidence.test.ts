import { describe, expect, it } from "bun:test";
import type {
  EvidenceFilter,
  EvidenceRow,
  EvidenceStorePort,
  TransitionEvidenceRow,
} from "@/v2/repo/evidence-store.port.js";
import { emitTransitionEvidence } from "@/v2/service/emit-transition-evidence.js";

function makeStore(): { store: EvidenceStorePort; rows: EvidenceRow[] } {
  const rows: EvidenceRow[] = [];
  const store: EvidenceStorePort = {
    async append(row) {
      rows.push(row);
    },
    async list(_filter?: EvidenceFilter) {
      return rows;
    },
  };
  return { store, rows };
}

describe("emitTransitionEvidence", () => {
  it("stamps kind=transition, generated id, and clock-provided timestamp", async () => {
    const { store, rows } = makeStore();
    const FROZEN = new Date("2026-05-15T08:00:00.000Z");
    let n = 0;
    const result = await emitTransitionEvidence(
      { store, clock: () => FROZEN, idFactory: () => `evd-${++n}` },
      {
        task_id: "tsk-1",
        from_state: "draft",
        to_state: "claimed",
        trigger_verb: "task:claim",
      },
    );
    expect(result.kind).toBe("transition");
    expect(result.id).toBe("evd-1");
    expect(result.timestamp).toBe(FROZEN.toISOString());
    expect(result.from_state).toBe("draft");
    expect(result.to_state).toBe("claimed");
    expect(rows.length).toBe(1);
    expect(rows[0]).toEqual(result);
  });

  it("passes through optional fields verdict, agent_id, reason", async () => {
    const { store } = makeStore();
    const result = (await emitTransitionEvidence(
      { store },
      {
        task_id: "tsk-2",
        from_state: "verifying",
        to_state: "doing",
        trigger_verb: "task:verify",
        verdict: "FAIL",
        agent_id: "agent-a",
        reason: "lint-violation",
      },
    )) as TransitionEvidenceRow;
    expect(result.verdict).toBe("FAIL");
    expect(result.agent_id).toBe("agent-a");
    expect(result.reason).toBe("lint-violation");
  });
});
