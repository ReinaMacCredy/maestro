import { describe, it, expect } from "bun:test";
import { postPrCheck } from "@/features/ci/usecases/post-pr-check.js";
import type { PostPrCheckArgs, PostPrCheckDeps } from "@/features/ci/usecases/post-pr-check.js";
import type { GithubApiPort, CheckRunInput } from "@/features/ci/ports/github-api.port.js";
import type { Verdict, VerdictDecision } from "@/features/verdict/domain/types.js";
import { generateVerdictId } from "@/features/verdict/domain/verdict-id.js";

// ─── Factories ────────────────────────────────────────────────────────────────

function makeVerdict(
  decision: VerdictDecision = "PASS",
  overrides: Partial<Verdict> = {},
): Verdict {
  return {
    schemaVersion: 1,
    id: generateVerdictId(),
    taskId: "tsk-aaaaaa",
    contractVersion: 1,
    computedAt: new Date().toISOString(),
    decision,
    effectiveRiskClass: "medium",
    reasons: [{ category: "policy", code: "all-checks-passed", message: "all checks passed" }],
    evidenceConsulted: [],
    policiesConsulted: [],
    trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
    ...overrides,
  };
}

interface FakePortState {
  posted: (CheckRunInput & { checkRunId?: never })[];
  patched: (CheckRunInput & { checkRunId: number })[];
  nextId: number;
}

function fakeGithubApi(state: FakePortState): GithubApiPort {
  return {
    postCheckRun: async (input) => {
      state.posted.push(input);
      return { id: state.nextId++ };
    },
    patchCheckRun: async (input) => {
      state.patched.push(input);
    },
  };
}

function makeDeps(state: FakePortState): PostPrCheckDeps {
  return { githubApi: fakeGithubApi(state) };
}

function makeArgs(
  decision: VerdictDecision = "PASS",
  overrides: Partial<PostPrCheckArgs> = {},
): PostPrCheckArgs {
  return {
    verdict: makeVerdict(decision),
    repository: "owner/repo",
    headSha: "abc1234",
    ...overrides,
  };
}

// ─── Conclusion mapping ───────────────────────────────────────────────────────

describe("postPrCheck — conclusion mapping", () => {
  const cases: [VerdictDecision, string][] = [
    ["PASS", "success"],
    ["FAIL", "failure"],
    ["BLOCK", "failure"],
    ["HUMAN", "action_required"],
  ];

  for (const [decision, expectedConclusion] of cases) {
    it(`${decision} → conclusion "${expectedConclusion}"`, async () => {
      const state: FakePortState = { posted: [], patched: [], nextId: 1 };
      await postPrCheck(makeArgs(decision), makeDeps(state));
      expect(state.posted[0]?.conclusion).toBe(expectedConclusion);
    });
  }
});

// ─── POST vs PATCH routing ────────────────────────────────────────────────────

describe("postPrCheck — POST vs PATCH routing", () => {
  it("calls postCheckRun when no existingCheckRunId provided; returns new id", async () => {
    const state: FakePortState = { posted: [], patched: [], nextId: 42 };
    const result = await postPrCheck(makeArgs(), makeDeps(state));
    expect(state.posted).toHaveLength(1);
    expect(state.patched).toHaveLength(0);
    expect(result.checkRunId).toBe(42);
  });

  it("calls patchCheckRun when existingCheckRunId is provided; returns same id", async () => {
    const state: FakePortState = { posted: [], patched: [], nextId: 99 };
    const result = await postPrCheck(
      makeArgs("PASS", { existingCheckRunId: 7 }),
      makeDeps(state),
    );
    expect(state.posted).toHaveLength(0);
    expect(state.patched).toHaveLength(1);
    expect(state.patched[0]?.checkRunId).toBe(7);
    expect(result.checkRunId).toBe(7);
  });
});

// ─── Payload content ──────────────────────────────────────────────────────────

describe("postPrCheck — payload content", () => {
  it("title includes the decision", async () => {
    const state: FakePortState = { posted: [], patched: [], nextId: 1 };
    await postPrCheck(makeArgs("FAIL"), makeDeps(state));
    expect(state.posted[0]?.title).toBe("Maestro Verdict: FAIL");
  });

  it("name is 'Maestro Verify'", async () => {
    const state: FakePortState = { posted: [], patched: [], nextId: 1 };
    await postPrCheck(makeArgs(), makeDeps(state));
    expect(state.posted[0]?.name).toBe("Maestro Verify");
  });

  it("summary contains effectiveRiskClass", async () => {
    const state: FakePortState = { posted: [], patched: [], nextId: 1 };
    const verdict = makeVerdict("PASS", { effectiveRiskClass: "high" });
    await postPrCheck({ verdict, repository: "owner/repo", headSha: "abc" }, makeDeps(state));
    expect(state.posted[0]?.summary).toContain("high");
  });

  it("summary contains at least one reason message", async () => {
    const state: FakePortState = { posted: [], patched: [], nextId: 1 };
    const verdict = makeVerdict("PASS", {
      reasons: [
        { category: "policy", code: "all-checks-passed", message: "everything looks good" },
      ],
    });
    await postPrCheck({ verdict, repository: "owner/repo", headSha: "abc" }, makeDeps(state));
    expect(state.posted[0]?.summary).toContain("everything looks good");
  });

  it("headSha and repository are forwarded to postCheckRun", async () => {
    const state: FakePortState = { posted: [], patched: [], nextId: 1 };
    await postPrCheck(
      { verdict: makeVerdict(), repository: "org/myrepo", headSha: "deadbeef" },
      makeDeps(state),
    );
    expect(state.posted[0]?.repository).toBe("org/myrepo");
    expect(state.posted[0]?.headSha).toBe("deadbeef");
  });
});
