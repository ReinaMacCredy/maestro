import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { Command } from "commander";
import { registerMergeAutoCommand } from "@/features/merge/commands/merge-auto.command.js";
import type { Verdict } from "@/features/verdict/domain/types.js";
import type { VerdictStorePort } from "@/features/verdict/ports/storage.js";
import type { EvidenceRow, EvidenceStorePort } from "@/features/evidence/index.js";
import type { ContractVersionStorePort } from "@/features/task/ports/contract-version-store.port.js";
import type { ContractStorePort } from "@/features/task/ports/contract-store.port.js";
import type { GitAnchorPort } from "@/features/task/ports/git-anchor.port.js";
import type { GithubApiPort } from "@/features/ci/ports/github-api.port.js";
import type { AutopilotPolicy } from "@/features/policy/index.js";
import type { Contract } from "@/features/task/index.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";
import type { LegacySpecStorePort as SpecStorePort } from "@/shared/domain/legacy-spec/index.js";

// ─── Console / process capture ────────────────────────────────────────────────

const originalConsoleLog = console.log;
const originalProcessStdoutWrite = process.stdout.write.bind(process.stdout);
const originalProcessExit = process.exit;

// Suppress actual process.exit so a failing test does not kill the runner.
// Individual tests override this as needed.
beforeEach(() => {
  process.exit = (code?: number) => {
    throw new Error(`exit:${String(code ?? 0)}`);
  };
});

function captureStdout(): { lines: string[] } {
  const lines: string[] = [];
  console.log = (...args: unknown[]) => { lines.push(args.map(String).join(" ")); };
  process.stdout.write = (chunk: string | Uint8Array): boolean => {
    lines.push(typeof chunk === "string" ? chunk : Buffer.from(chunk).toString("utf8"));
    return true;
  };
  return { lines };
}

afterEach(() => {
  console.log = originalConsoleLog;
  process.stdout.write = originalProcessStdoutWrite;
  process.exit = originalProcessExit;
});

// ─── Fixtures ─────────────────────────────────────────────────────────────────

function makeVerdict(overrides: Partial<Verdict> = {}): Verdict {
  return {
    schemaVersion: 1,
    id: "vrd-test-001",
    taskId: "tsk-aaaaaa",
    contractVersion: 1,
    computedAt: "2026-05-05T00:00:00.000Z",
    decision: "PASS",
    effectiveRiskClass: "low",
    reasons: [],
    evidenceConsulted: [],
    policiesConsulted: [],
    trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
    // merge-auto filters by tree_sha + pr; default to the test fixtures'
    // values so the eligibility flow can locate the verdict.
    subject: { tree_sha: "deadbeef", pr: 42 },
    ...overrides,
  };
}

function makeContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "ctr-test-001",
    taskId: "tsk-aaaaaa",
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-05-05T00:00:00.000Z",
    intent: "Test contract",
    scope: { filesExpected: ["src/**"], filesForbidden: [] },
    doneWhen: [],
    amendments: [],
    createdBy: "agent",
    configSnapshot: {
      strict: true,
      overlapPolicy: "fail",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    ...overrides,
  };
}

function makeAutopilotPolicy(overrides: Partial<AutopilotPolicy> = {}): AutopilotPolicy {
  return {
    id: "autopilot-default",
    kind: "autopilot",
    autoMergeAllowed: { low: true, medium: true, high: false, critical: false },
    requiredWitnessLevel: {
      low: "agent-claimed-locally",
      medium: "witnessed-by-ci",
      high: "witnessed-by-ci",
      critical: "witnessed-by-maestro",
    },
    version: "1",
    ...overrides,
  };
}

function fakeVerdictStore(verdict?: Verdict): VerdictStorePort {
  return {
    write: async () => {},
    readLatest: async () => verdict,
    readVersion: async () => verdict,
    history: async () => (verdict ? [verdict] : []),
    findByTreeSha: async () => (verdict ? [verdict] : []),
  };
}

function fakeContractVersionStore(contract?: Contract): ContractVersionStorePort {
  return {
    write: async () => {},
    readCurrent: async () => contract,
    readVersion: async () => contract,
    history: async () => (contract ? [contract] : []),
  };
}

function fakeEvidenceStore(rows: EvidenceRow[] = []): EvidenceStorePort {
  return {
    append: async () => {},
    read: async () => undefined,
    list: async (filter = {}) => {
      if (filter.task_id) return rows.filter((r) => r.task_id === filter.task_id);
      return rows;
    },
  };
}

/**
 * Minimal evidence row that passes the rollback-not-witnessed check.
 * All other eligibility checks are satisfied by the PASS/low verdict +
 * auto-merge enabled for low risk class.
 */
function makeRollbackEvidenceRow(): EvidenceRow<"rollback-exercised"> {
  return {
    schema_version: 1,
    id: "ev-rollback-001",
    task_id: "tsk-aaaaaa",
    kind: "rollback-exercised",
    witness_level: "witnessed-by-ci",
    created_at: "2026-05-05T00:00:00.000Z",
    payload: { command: "bun test:rollback", exit: 0 },
  };
}

function fakeGitAnchor(): GitAnchorPort {
  return {
    resolveRepoRoot: async (cwd) => cwd,
    resolveHeadCommit: async () => "abc1234",
    collectTouchedFiles: async () => ({ gitAvailable: true, actualFilesTouched: [] }),
    windowsOverlap: async () => false,
    collectChangedPaths: async () => [],
    collectAddedLines: async () => [],
    collectUntrackedFiles: async () => [],
    resolveTreeSha: async () => "deadbeef",
  };
}

function fakeSpecStore(): SpecStorePort {
  return {
    write: async () => {},
    read: async () => undefined,
    list: async () => [],
  };
}

interface FakeMergeServices {
  verdictStore: VerdictStorePort;
  evidenceStore: EvidenceStorePort;
  contractVersionStore: ContractVersionStorePort;
  contractStore: ContractStorePort;
  gitAnchor: GitAnchorPort;
  getEffectiveAutopilotPolicy: () => Promise<AutopilotPolicy>;
  specStore: SpecStorePort;
  githubApi: GithubApiPort;
  projectRoot: string;
}

function makeFakeGithubApi(): { api: GithubApiPort; calls: string[] } {
  const calls: string[] = [];
  const api: GithubApiPort = {
    getPullRequestAuthor: async () => "test-user",
    postCheckRun: async () => ({ id: 1 }),
    patchCheckRun: async () => {},
    triggerAutoMerge: async (input) => {
      calls.push(`triggerAutoMerge:${input.repository}:${input.pr}`);
    },
    listOpenPullRequests: async () => [],
    getPullRequestFiles: async () => [],
  };
  return { api, calls };
}

function makeEligibleServices(): { services: FakeMergeServices; githubApiCalls: string[] } {
  const { api, calls } = makeFakeGithubApi();
  // An eligible verdict: PASS, low risk, auto-merge allowed for low.
  // Include rollback evidence to satisfy the rollback-not-witnessed check.
  const verdict = makeVerdict({ decision: "PASS", effectiveRiskClass: "low" });
  const services: FakeMergeServices = {
    verdictStore: fakeVerdictStore(verdict),
    evidenceStore: fakeEvidenceStore([makeRollbackEvidenceRow()]),
    contractVersionStore: fakeContractVersionStore(makeContract()),
    contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
    gitAnchor: fakeGitAnchor(),
    getEffectiveAutopilotPolicy: async () => makeAutopilotPolicy(),
    specStore: fakeSpecStore(),
    githubApi: api,
    projectRoot: "/tmp/test-project",
  };
  return { services, githubApiCalls: calls };
}

function makeIneligibleServices(): { services: FakeMergeServices; githubApiCalls: string[] } {
  const { api, calls } = makeFakeGithubApi();
  // An ineligible verdict: FAIL
  const verdict = makeVerdict({ decision: "FAIL", effectiveRiskClass: "low" });
  const services: FakeMergeServices = {
    verdictStore: fakeVerdictStore(verdict),
    evidenceStore: fakeEvidenceStore(),
    contractVersionStore: fakeContractVersionStore(makeContract()),
    contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
    gitAnchor: fakeGitAnchor(),
    getEffectiveAutopilotPolicy: async () => makeAutopilotPolicy(),
    specStore: fakeSpecStore(),
    githubApi: api,
    projectRoot: "/tmp/test-project",
  };
  return { services, githubApiCalls: calls };
}

function makeProgram(services: FakeMergeServices): Command {
  const program = new Command()
    .name("maestro")
    .option("--json")
    .exitOverride();
  const mergeCmd = program
    .command("merge")
    .description("Merge controls");
  registerMergeAutoCommand(mergeCmd, program, { getServices: () => services });
  return program;
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("merge auto — eligible PR", () => {
  let savedEnv: string | undefined;

  beforeEach(() => {
    savedEnv = process.env.GITHUB_REPOSITORY;
    process.env.GITHUB_REPOSITORY = "owner/repo";
  });

  afterEach(() => {
    if (savedEnv === undefined) {
      delete process.env.GITHUB_REPOSITORY;
    } else {
      process.env.GITHUB_REPOSITORY = savedEnv;
    }
  });

  it("calls triggerAutoMerge exactly once and exits 0", async () => {
    const { services, githubApiCalls } = makeEligibleServices();
    const program = makeProgram(services);
    captureStdout();

    await program.parseAsync(["node", "maestro", "merge", "auto", "--pr", "42", "--task", "tsk-aaaaaa"]);

    expect(githubApiCalls).toHaveLength(1);
    expect(githubApiCalls[0]).toContain("triggerAutoMerge");
    expect(githubApiCalls[0]).toContain("42");
  });

  it("outputs [ok] message on success (plain text)", async () => {
    const { services } = makeEligibleServices();
    const program = makeProgram(services);
    const { lines } = captureStdout();

    await program.parseAsync(["node", "maestro", "merge", "auto", "--pr", "42", "--task", "tsk-aaaaaa"]);

    const combined = lines.join("\n");
    expect(combined).toContain("[ok]");
    expect(combined).toContain("42");
  });

  it("--json mode emits { eligible: true, reasons: [], merged: true }", async () => {
    const { services } = makeEligibleServices();
    const program = makeProgram(services);
    const { lines } = captureStdout();

    await program.parseAsync(["node", "maestro", "merge", "auto", "--pr", "42", "--task", "tsk-aaaaaa", "--json"]);

    const raw = lines.join("");
    const parsed = JSON.parse(raw) as { eligible: boolean; reasons: unknown[]; merged: boolean };
    expect(parsed.eligible).toBe(true);
    expect(parsed.reasons).toHaveLength(0);
    expect(parsed.merged).toBe(true);
  });
});

describe("merge auto — ineligible PR", () => {
  let savedEnv: string | undefined;

  beforeEach(() => {
    savedEnv = process.env.GITHUB_REPOSITORY;
    process.env.GITHUB_REPOSITORY = "owner/repo";
  });

  afterEach(() => {
    if (savedEnv === undefined) {
      delete process.env.GITHUB_REPOSITORY;
    } else {
      process.env.GITHUB_REPOSITORY = savedEnv;
    }
  });

  it("never calls triggerAutoMerge and exits 1", async () => {
    const { services, githubApiCalls } = makeIneligibleServices();
    const program = makeProgram(services);
    captureStdout();

    let capturedExitCode: number | undefined;
    process.exit = (code?: number) => {
      capturedExitCode = code;
      throw new Error(`exit:${String(code)}`);
    };

    try {
      await program.parseAsync(["node", "maestro", "merge", "auto", "--pr", "42", "--task", "tsk-aaaaaa"]);
    } catch (err) {
      const msg = (err as Error).message;
      if (!msg.startsWith("exit:")) throw err;
    }

    expect(githubApiCalls).toHaveLength(0);
    expect(capturedExitCode).toBe(1);
  });

  it("prints itemized reasons on ineligible (plain text)", async () => {
    const { services } = makeIneligibleServices();
    const program = makeProgram(services);
    const { lines } = captureStdout();

    try {
      await program.parseAsync(["node", "maestro", "merge", "auto", "--pr", "42", "--task", "tsk-aaaaaa"]);
    } catch (err) {
      const msg = (err as Error).message;
      if (!msg.startsWith("exit:")) throw err;
    }

    const combined = lines.join("\n");
    // Should contain the verdict-not-pass reason code (FAIL verdict is ineligible)
    expect(combined).toContain("[verdict-not-pass]");
  });

  it("--json mode emits { eligible: false, reasons: [...], merged: false }", async () => {
    const { services } = makeIneligibleServices();
    const program = makeProgram(services);
    const { lines } = captureStdout();

    process.exit = () => { throw new Error("exit:1"); };

    try {
      await program.parseAsync(["node", "maestro", "merge", "auto", "--pr", "42", "--task", "tsk-aaaaaa", "--json"]);
    } catch (err) {
      const msg = (err as Error).message;
      if (!msg.startsWith("exit:")) throw err;
    }

    const raw = lines.join("");
    const parsed = JSON.parse(raw) as { eligible: boolean; reasons: unknown[]; merged: boolean };
    expect(parsed.eligible).toBe(false);
    expect(Array.isArray(parsed.reasons)).toBe(true);
    expect(parsed.reasons.length).toBeGreaterThan(0);
    expect(parsed.merged).toBe(false);
  });
});

describe("merge auto — verdict identity is bound to (pr, tree_sha)", () => {
  let savedEnv: string | undefined;

  beforeEach(() => {
    savedEnv = process.env.GITHUB_REPOSITORY;
    process.env.GITHUB_REPOSITORY = "owner/repo";
  });

  afterEach(() => {
    if (savedEnv === undefined) {
      delete process.env.GITHUB_REPOSITORY;
    } else {
      process.env.GITHUB_REPOSITORY = savedEnv;
    }
  });

  it("refuses to reuse a PASS verdict whose tree_sha does not match the current HEAD", async () => {
    // Stale verdict from a different tree (e.g., older content before a force-push).
    const staleVerdict = makeVerdict({
      decision: "PASS",
      effectiveRiskClass: "low",
      subject: { tree_sha: "stale123", pr: 42 },
    });
    const { api, calls } = makeFakeGithubApi();
    const services: FakeMergeServices = {
      verdictStore: {
        write: async () => {},
        readLatest: async () => staleVerdict,
        readVersion: async () => staleVerdict,
        history: async () => [staleVerdict],
        // Crucially, current tree (deadbeef) does not match staleVerdict.subject.tree_sha
        findByTreeSha: async (sha) => sha === "stale123" ? [staleVerdict] : [],
      },
      evidenceStore: fakeEvidenceStore([makeRollbackEvidenceRow()]),
      contractVersionStore: fakeContractVersionStore(makeContract()),
      contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
      gitAnchor: fakeGitAnchor(),
      getEffectiveAutopilotPolicy: async () => makeAutopilotPolicy(),
      specStore: fakeSpecStore(),
      githubApi: api,
      projectRoot: "/tmp/test-project",
    };
    const program = makeProgram(services);
    captureStdout();

    let thrown: Error | undefined;
    try {
      await program.parseAsync(["node", "maestro", "merge", "auto", "--pr", "42", "--task", "tsk-aaaaaa"]);
    } catch (err) {
      thrown = err as Error;
    }

    expect(thrown).toBeDefined();
    expect(thrown?.message).toContain("No verdict found for task tsk-aaaaaa on PR 42");
    expect(calls).toHaveLength(0);
  });

  it("refuses to reuse a verdict tagged with a different PR number even if the tree matches", async () => {
    // Verdict for the right tree but wrong PR — e.g. someone re-targeted the branch.
    const wrongPrVerdict = makeVerdict({
      decision: "PASS",
      effectiveRiskClass: "low",
      subject: { tree_sha: "deadbeef", pr: 99 },
    });
    const { api, calls } = makeFakeGithubApi();
    const services: FakeMergeServices = {
      verdictStore: {
        write: async () => {},
        readLatest: async () => wrongPrVerdict,
        readVersion: async () => wrongPrVerdict,
        history: async () => [wrongPrVerdict],
        findByTreeSha: async () => [wrongPrVerdict],
      },
      evidenceStore: fakeEvidenceStore([makeRollbackEvidenceRow()]),
      contractVersionStore: fakeContractVersionStore(makeContract()),
      contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
      gitAnchor: fakeGitAnchor(),
      getEffectiveAutopilotPolicy: async () => makeAutopilotPolicy(),
      specStore: fakeSpecStore(),
      githubApi: api,
      projectRoot: "/tmp/test-project",
    };
    const program = makeProgram(services);
    captureStdout();

    let thrown: Error | undefined;
    try {
      await program.parseAsync(["node", "maestro", "merge", "auto", "--pr", "42", "--task", "tsk-aaaaaa"]);
    } catch (err) {
      thrown = err as Error;
    }

    expect(thrown).toBeDefined();
    expect(thrown?.message).toContain("No verdict found for task tsk-aaaaaa on PR 42");
    expect(calls).toHaveLength(0);
  });
});
