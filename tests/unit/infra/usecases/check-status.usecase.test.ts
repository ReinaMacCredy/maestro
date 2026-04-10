import { describe, expect, it } from "bun:test";
import type { UkiHandoff } from "@/features/handoff";
import { checkStatus } from "@/infra/usecases/check-status.usecase.js";
import { mockConfig, mockGit } from "../../../helpers/mocks.js";
import type { HandoffStorePort } from "@/features/handoff";

function makeHandoffStore(count: number): HandoffStorePort {
  const handoffs: UkiHandoff[] = Array.from({ length: count }, (_, index) => ({
    id: `2026-04-09-00${index + 1}`,
    version: "5.4",
    timestamp: "2026-04-09T00:00:00.000Z",
    status: "pending",
    agent: "codex",
    sessionId: "session-1",
    content: {
      mode: "execute",
      currentState: "execute_in_progress",
      sessionCore: `status_test_${index + 1}`,
      decisions: [],
      artifacts: ["branch_main"],
      readMore: ["file_src_status_ts"],
      nextAction: "inspect_status",
      summary: `Status_test_${index + 1}`,
      maestroRefs: {},
      cs: { work: 0.9 },
      signalDelta: [],
      boundaryState: [],
      risks: [],
      causalDrivers: [],
      divergences: [],
      touchedFiles: ["file_src_status_ts"],
      completedWork: [],
      validation: [],
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
  it("reports pending handoffs from the handoff store when requested", async () => {
    const status = await checkStatus(
      mockConfig({ exists: async () => true }),
      mockGit(),
      makeHandoffStore(2),
      process.cwd(),
      { includePendingHandoffs: true },
    );

    expect(status.pendingHandoffs).toHaveLength(2);
    expect(status.pendingHandoffs[0]).toEqual({
      id: "2026-04-09-001",
      agent: "codex",
      createdAt: "2026-04-09T00:00:00.000Z",
    });
  });

  it("skips pending handoff reads by default", async () => {
    let listCalls = 0;
    const store = makeHandoffStore(2);
    const handoffStore: HandoffStorePort = {
      ...store,
      list: async (filter) => {
        listCalls += 1;
        return store.list(filter);
      },
    };

    const status = await checkStatus(
      mockConfig({ exists: async () => true }),
      mockGit(),
      handoffStore,
      process.cwd(),
    );

    expect(listCalls).toBe(0);
    expect(status.pendingHandoffs).toEqual([]);
  });

  it("skips pending handoff reads when they are explicitly disabled", async () => {
    let listCalls = 0;
    const store = makeHandoffStore(2);
    const handoffStore: HandoffStorePort = {
      ...store,
      list: async (filter) => {
        listCalls += 1;
        return store.list(filter);
      },
    };

    const status = await checkStatus(
      mockConfig({ exists: async () => true }),
      mockGit(),
      handoffStore,
      process.cwd(),
      { includePendingHandoffs: false },
    );

    expect(listCalls).toBe(0);
    expect(status.pendingHandoffs).toEqual([]);
  });
});
