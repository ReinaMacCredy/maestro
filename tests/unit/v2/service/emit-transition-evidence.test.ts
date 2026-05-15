import { describe, expect, it } from "bun:test";
import type {
  EvidenceFilter,
  EvidenceRow,
  EvidenceStorePort,
  TransitionEvidenceRow,
} from "@/v2/repo/evidence-store.port.js";
import type {
  ObservabilityEvent,
  ObservabilityPort,
} from "@/v2/repo/observability.port.js";
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

function makeObservability(): {
  port: ObservabilityPort;
  events: ObservabilityEvent[];
} {
  const events: ObservabilityEvent[] = [];
  const port: ObservabilityPort = {
    async emit(event) {
      events.push(event);
    },
  };
  return { port, events };
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

  it("emits an observability event mirroring the evidence row when observabilityStore is supplied", async () => {
    const { store } = makeStore();
    const { port, events } = makeObservability();
    const FROZEN = new Date("2026-05-15T08:30:00.000Z");
    await emitTransitionEvidence(
      {
        store,
        observabilityStore: port,
        clock: () => FROZEN,
        idFactory: () => "evd-obs-1",
      },
      {
        task_id: "tsk-9",
        from_state: "claimed",
        to_state: "verifying",
        trigger_verb: "task:verify",
        verdict: "PASS",
        reason: "lint-clean",
      },
    );
    expect(events).toHaveLength(1);
    const event = events[0]!;
    expect(event.task_id).toBe("tsk-9");
    expect(event.kind).toBe("transition");
    expect(event.timestamp).toBe(FROZEN.toISOString());
    expect(event.payload).toEqual({
      evidence_id: "evd-obs-1",
      from_state: "claimed",
      to_state: "verifying",
      trigger_verb: "task:verify",
      verdict: "PASS",
      reason: "lint-clean",
    });
  });

  it("does not emit an observability event when the row has no task_id (plan-only transitions)", async () => {
    const { store } = makeStore();
    const { port, events } = makeObservability();
    await emitTransitionEvidence(
      { store, observabilityStore: port },
      {
        plan_id: "pln-1",
        from_state: "specified",
        to_state: "planned",
        trigger_verb: "plan:decompose",
      },
    );
    expect(events).toHaveLength(0);
  });

  it("is a no-op for observability when observabilityStore is omitted", async () => {
    const { store, rows } = makeStore();
    await emitTransitionEvidence(
      { store },
      {
        task_id: "tsk-3",
        from_state: "draft",
        to_state: "claimed",
        trigger_verb: "task:claim",
      },
    );
    expect(rows).toHaveLength(1);
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
