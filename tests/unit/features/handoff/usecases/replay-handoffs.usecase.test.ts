import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsHandoffStoreAdapter, loadPriorHandoffs } from "@/features/handoff";
import type {
  ExecuteUkiHandoffContent,
  HandoffStorePort,
  PlanUkiHandoffContent,
  UkiHandoff,
} from "@/features/handoff";
import type { SessionDetectPort, AgentSession } from "@/features/session";
import { createUkiHandoff } from "@/features/handoff";

class StubSessionDetect implements SessionDetectPort {
  async detect(): Promise<AgentSession | undefined> {
    return { agent: "test", sessionId: "test-session", sourcePath: "/tmp/fake.jsonl" };
  }

  async lookup(): Promise<AgentSession | undefined> {
    return undefined;
  }
}

function makeExecuteContent(
  missionId: string,
  featureId: string,
  overrides: Partial<ExecuteUkiHandoffContent> = {},
): ExecuteUkiHandoffContent {
  return {
    mode: "execute",
    currentState: "done",
    sessionCore: "test",
    decisions: [],
    artifacts: ["branch_test"],
    readMore: ["file_test_ts"],
    nextAction: "next",
    summary: "Test summary",
    maestroRefs: { missionId, featureId },
    cs: { work: 0.9 },
    signalDelta: [],
    boundaryState: [],
    risks: [],
    causalDrivers: [],
    divergences: [],
    touchedFiles: ["file_test_ts"],
    completedWork: [],
    validation: ["unit_green"],
    ...overrides,
  };
}

function makePlanContent(
  missionId: string,
  featureId: string,
  overrides: Partial<PlanUkiHandoffContent> = {},
): PlanUkiHandoffContent {
  return {
    mode: "plan",
    currentState: "planned",
    sessionCore: "test",
    decisions: [],
    artifacts: ["branch_test"],
    readMore: ["file_test_ts"],
    nextAction: "implement",
    summary: "Plan summary",
    maestroRefs: { missionId, featureId },
    cs: { summary: 0.85 },
    signalDelta: [],
    boundaryState: [],
    risks: [],
    causalDrivers: [],
    divergences: [],
    planPaths: [],
    maestroSync: [],
    ...overrides,
  };
}

describe("loadPriorHandoffs", () => {
  const missionA = "mission_a";
  const missionB = "mission_b";
  let dir: string;
  let store: FsHandoffStoreAdapter;
  const detect = new StubSessionDetect();

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "maestro-replay-"));
    store = new FsHandoffStoreAdapter(dir);
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("returns summaries for matching featureId", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent(missionA, "f1", { risks: ["race condition"] }),
    });
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent(missionA, "f2", { risks: ["unrelated"] }),
      });

      const result = await loadPriorHandoffs(store, missionA, "f1");
      expect(result).toBeDefined();
      if (!result) throw new Error("Expected prior handoffs");
      expect(result).toHaveLength(1);
      expect(result[0]?.risks).toEqual(["race condition"]);
  });

  it("returns undefined when no handoffs match", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent(missionA, "f2", { risks: ["something"] }),
    });

    const result = await loadPriorHandoffs(store, missionA, "f1");
    expect(result).toBeUndefined();
  });

  it("returns undefined for empty store", async () => {
    const result = await loadPriorHandoffs(store, missionA, "f1");
    expect(result).toBeUndefined();
  });

  it("caps at 3 most recent", async () => {
    for (let i = 0; i < 5; i++) {
      await createUkiHandoff(store, detect, dir, {
        content: makeExecuteContent(missionA, "f1", { risks: [`risk_${i}`] }),
      });
    }

    const result = await loadPriorHandoffs(store, missionA, "f1");
    expect(result).toHaveLength(3);
  });

  it("skips handoffs with all-empty replay fields", async () => {
    // This handoff has matching featureId but no replay-worthy content
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent(missionA, "f1"),
    });
    // This one has risks
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent(missionA, "f1", { risks: ["actual risk"] }),
    });

      const result = await loadPriorHandoffs(store, missionA, "f1");
      expect(result).toBeDefined();
      if (!result) throw new Error("Expected prior handoffs");
      expect(result).toHaveLength(1);
      expect(result[0]?.risks).toEqual(["actual risk"]);
  });

  it("includes completedWork for execute mode", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent(missionA, "f1", {
        risks: ["risk"],
        completedWork: ["implemented retry logic"],
      }),
    });

      const result = await loadPriorHandoffs(store, missionA, "f1");
      expect(result).toBeDefined();
      if (!result) throw new Error("Expected prior handoffs");
      expect(result[0]?.completedWork).toEqual(["implemented retry logic"]);
  });

  it("omits completedWork for plan mode", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makePlanContent(missionA, "f1", { risks: ["design risk"] }),
    });

      const result = await loadPriorHandoffs(store, missionA, "f1");
      expect(result).toBeDefined();
      if (!result) throw new Error("Expected prior handoffs");
      expect(result[0]?.completedWork).toBeUndefined();
      expect(result[0]?.mode).toBe("plan");
  });

  it("preserves newest-first order", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent(missionA, "f1", { risks: ["first"] }),
    });
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent(missionA, "f1", { risks: ["second"] }),
    });
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent(missionA, "f1", { risks: ["third"] }),
    });

      const result = await loadPriorHandoffs(store, missionA, "f1");
      expect(result).toHaveLength(3);
      // Store returns newest first, so "third" should be first in result
      if (!result) throw new Error("Expected prior handoffs");
      expect(result[0]?.risks).toEqual(["third"]);
      expect(result[2]?.risks).toEqual(["first"]);
  });

  it("includes assumptions and verificationResults when present", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent(missionA, "f1", {
        assumptions: ["Redis is available"],
        verificationResults: [
          { step: "build", passed: true },
          { step: "test", passed: false },
        ],
      }),
    });

      const result = await loadPriorHandoffs(store, missionA, "f1");
      expect(result).toBeDefined();
      if (!result) throw new Error("Expected prior handoffs");
      expect(result[0]?.assumptions).toEqual(["Redis is available"]);
      expect(result[0]?.verificationResults).toEqual([
        { step: "build", passed: true },
        { step: "test", passed: false },
      ]);
  });

  it("includes blindSpot and causalDrivers", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent(missionA, "f1", {
        blindSpot: "Did not test with cluster mode",
        causalDrivers: ["CI flake rate was 12%"],
      }),
    });

      const result = await loadPriorHandoffs(store, missionA, "f1");
      expect(result).toBeDefined();
      if (!result) throw new Error("Expected prior handoffs");
      expect(result[0]?.blindSpot).toBe("Did not test with cluster mode");
      expect(result[0]?.causalDrivers).toEqual(["CI flake rate was 12%"]);
  });

  it("ignores handoffs from other missions with the same featureId", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent(missionA, "f1", { risks: ["mission_a_risk"] }),
    });
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent(missionB, "f1", { risks: ["mission_b_risk"] }),
    });

      const result = await loadPriorHandoffs(store, missionA, "f1");
      expect(result).toHaveLength(1);
      if (!result) throw new Error("Expected prior handoffs");
      expect(result[0]?.risks).toEqual(["mission_a_risk"]);
  });

  it("uses the targeted handoff query instead of listing the full store", async () => {
    let targetedCalls = 0;
    let requestedLimit = 0;
    const matching = [
      {
        id: "2026-04-13-004",
        version: "5.4",
        timestamp: "2026-04-13T00:03:00.000Z",
        status: "pending",
        agent: "test",
        sessionId: "session",
        content: makeExecuteContent(missionA, "f1"),
        uki: "uki-4",
      },
      {
        id: "2026-04-13-003",
        version: "5.4",
        timestamp: "2026-04-13T00:02:00.000Z",
        status: "pending",
        agent: "test",
        sessionId: "session",
        content: makeExecuteContent(missionA, "f1"),
        uki: "uki-3",
      },
      {
        id: "2026-04-13-002",
        version: "5.4",
        timestamp: "2026-04-13T00:01:00.000Z",
        status: "pending",
        agent: "test",
        sessionId: "session",
        content: makeExecuteContent(missionA, "f1"),
        uki: "uki-2",
      },
      {
        id: "2026-04-13-001",
        version: "5.4",
        timestamp: "2026-04-13T00:00:00.000Z",
        status: "pending",
        agent: "test",
        sessionId: "session",
        content: makeExecuteContent(missionA, "f1", { risks: ["risk"] }),
        uki: "uki-1",
      },
    ] as const satisfies readonly UkiHandoff[];

    const handoffStore = {
      async create() {
        throw new Error("not used");
      },
      async claimPending() {
        throw new Error("not used");
      },
      async get() {
        throw new Error("not used");
      },
      async getLatestPending() {
        throw new Error("not used");
      },
      async list() {
        throw new Error("broad list should not be called");
      },
      async listRecentByFeatureRefs(
        _missionId: string,
        _featureId: string,
        limit: number,
      ) {
        targetedCalls += 1;
        requestedLimit = limit;
        return matching.slice(0, limit);
      },
      async updateStatus() {
        throw new Error("not used");
      },
      async delete() {
        throw new Error("not used");
      },
    } as unknown as HandoffStorePort;

      const result = await loadPriorHandoffs(handoffStore, missionA, "f1");
      expect(targetedCalls).toBe(1);
      expect(requestedLimit).toBe(Number.MAX_SAFE_INTEGER);
      expect(result).toHaveLength(1);
      if (!result) throw new Error("Expected prior handoffs");
      expect(result[0]?.risks).toEqual(["risk"]);
    });
  });
