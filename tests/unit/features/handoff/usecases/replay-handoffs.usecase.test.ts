import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsHandoffStoreAdapter, loadPriorHandoffs } from "@/features/handoff";
import type { ExecuteUkiHandoffContent, PlanUkiHandoffContent } from "@/features/handoff";
import type { SessionDetectPort, AgentSession } from "@/features/session";
import { createUkiHandoff } from "@/features/handoff";

class StubSessionDetect implements SessionDetectPort {
  async detect(): Promise<AgentSession | undefined> {
    return { agent: "test", sessionId: "test-session", sourcePath: "/tmp/fake.jsonl" };
  }
}

function makeExecuteContent(featureId: string, overrides: Partial<ExecuteUkiHandoffContent> = {}): ExecuteUkiHandoffContent {
  return {
    mode: "execute",
    currentState: "done",
    sessionCore: "test",
    decisions: [],
    artifacts: ["branch_test"],
    readMore: ["file_test_ts"],
    nextAction: "next",
    summary: "Test summary",
    maestroRefs: { featureId },
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

function makePlanContent(featureId: string, overrides: Partial<PlanUkiHandoffContent> = {}): PlanUkiHandoffContent {
  return {
    mode: "plan",
    currentState: "planned",
    sessionCore: "test",
    decisions: [],
    artifacts: ["branch_test"],
    readMore: ["file_test_ts"],
    nextAction: "implement",
    summary: "Plan summary",
    maestroRefs: { featureId },
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
      content: makeExecuteContent("f1", { risks: ["race condition"] }),
    });
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent("f2", { risks: ["unrelated"] }),
    });

    const result = await loadPriorHandoffs(store, "f1");
    expect(result).toBeDefined();
    expect(result).toHaveLength(1);
    expect(result![0].risks).toEqual(["race condition"]);
  });

  it("returns undefined when no handoffs match", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent("f2", { risks: ["something"] }),
    });

    const result = await loadPriorHandoffs(store, "f1");
    expect(result).toBeUndefined();
  });

  it("returns undefined for empty store", async () => {
    const result = await loadPriorHandoffs(store, "f1");
    expect(result).toBeUndefined();
  });

  it("caps at 3 most recent", async () => {
    for (let i = 0; i < 5; i++) {
      await createUkiHandoff(store, detect, dir, {
        content: makeExecuteContent("f1", { risks: [`risk_${i}`] }),
      });
    }

    const result = await loadPriorHandoffs(store, "f1");
    expect(result).toHaveLength(3);
  });

  it("skips handoffs with all-empty replay fields", async () => {
    // This handoff has matching featureId but no replay-worthy content
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent("f1"),
    });
    // This one has risks
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent("f1", { risks: ["actual risk"] }),
    });

    const result = await loadPriorHandoffs(store, "f1");
    expect(result).toHaveLength(1);
    expect(result![0].risks).toEqual(["actual risk"]);
  });

  it("includes completedWork for execute mode", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent("f1", {
        risks: ["risk"],
        completedWork: ["implemented retry logic"],
      }),
    });

    const result = await loadPriorHandoffs(store, "f1");
    expect(result![0].completedWork).toEqual(["implemented retry logic"]);
  });

  it("omits completedWork for plan mode", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makePlanContent("f1", { risks: ["design risk"] }),
    });

    const result = await loadPriorHandoffs(store, "f1");
    expect(result![0].completedWork).toBeUndefined();
    expect(result![0].mode).toBe("plan");
  });

  it("preserves newest-first order", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent("f1", { risks: ["first"] }),
    });
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent("f1", { risks: ["second"] }),
    });
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent("f1", { risks: ["third"] }),
    });

    const result = await loadPriorHandoffs(store, "f1");
    expect(result).toHaveLength(3);
    // Store returns newest first, so "third" should be first in result
    expect(result![0].risks).toEqual(["third"]);
    expect(result![2].risks).toEqual(["first"]);
  });

  it("includes assumptions and verificationResults when present", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent("f1", {
        assumptions: ["Redis is available"],
        verificationResults: [
          { step: "build", passed: true },
          { step: "test", passed: false },
        ],
      }),
    });

    const result = await loadPriorHandoffs(store, "f1");
    expect(result![0].assumptions).toEqual(["Redis is available"]);
    expect(result![0].verificationResults).toEqual([
      { step: "build", passed: true },
      { step: "test", passed: false },
    ]);
  });

  it("includes blindSpot and causalDrivers", async () => {
    await createUkiHandoff(store, detect, dir, {
      content: makeExecuteContent("f1", {
        blindSpot: "Did not test with cluster mode",
        causalDrivers: ["CI flake rate was 12%"],
      }),
    });

    const result = await loadPriorHandoffs(store, "f1");
    expect(result![0].blindSpot).toBe("Did not test with cluster mode");
    expect(result![0].causalDrivers).toEqual(["CI flake rate was 12%"]);
  });
});
