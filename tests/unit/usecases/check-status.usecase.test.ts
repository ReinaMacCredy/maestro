import { describe, expect, it } from "bun:test";
import { checkStatus } from "../../../src/usecases/check-status.usecase.js";
import { mockConfig, mockGit } from "../../helpers/mocks.js";
import type { HandoffStorePort } from "../../../src/ports/handoff-store.port.js";
import type { UkiHandoff } from "../../../src/domain/uki-types.js";

function makeHandoffStore(count: number): HandoffStorePort {
  const handoffs: UkiHandoff[] = Array.from({ length: count }, (_, index) => ({
    id: `2026-04-09-00${index + 1}`,
    version: "5.2" as const,
    timestamp: "2026-04-09T00:00:00.000Z",
    status: "pending" as const,
    agent: "codex",
    sessionId: "session-1",
    slots: {
      sessionCore: `status_test_${index + 1}`,
      causalDrivers: [],
      divergences: [],
      keyDecisions: [],
      signalDelta: [],
      artifacts: ["branch_main"],
      executionState: "clean_tree",
      boundaryState: [],
      stanceCollapse: "NONE_DETECTED_LOW_FRICTION",
      nextAction: "inspect_status",
      cs: { work: 0.9 },
      summary: `Status_test_${index + 1}`,
    },
    uki: `uki-${index + 1}`,
  }));

  return {
    create: async () => {
      throw new Error("not used");
    },
    claimPending: async () => undefined,
    get: async () => undefined,
    getLatestPending: async () => handoffs[0],
    list: async (filter) => filter?.status
      ? handoffs.filter((handoff) => handoff.status === filter.status)
      : handoffs,
    updateStatus: async () => undefined,
    delete: async () => false,
  };
}

describe("checkStatus", () => {
  it("reports pending handoffs from the UKI handoff store", async () => {
    const status = await checkStatus(
      mockConfig({ exists: async () => true }),
      mockGit(),
      makeHandoffStore(2),
      process.cwd(),
    );

    expect(status.pendingHandoffs).toHaveLength(2);
  });
});
