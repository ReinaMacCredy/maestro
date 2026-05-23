import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { Command } from "commander";
import { registerCiVerifyCommand } from "@/features/ci/commands/ci-verify.command.js";
import type { Verdict, VerdictDecision } from "@/features/verdict/domain/types.js";
import type { VerdictStorePort } from "@/features/verdict/ports/storage.js";
import { generateVerdictId } from "@/features/verdict/domain/verdict-id.js";
import type { ContractVersionStorePort, ContractStorePort, GitAnchorPort, RunStateStorePort } from "@/shared/domain/task";
import type { EvidenceStorePort } from "@/features/evidence/ports/storage.js";
import type { LegacySpecStorePort as SpecStorePort } from "@/shared/domain/legacy-spec/index.js";
import type { GithubApiPort } from "@/features/ci/ports/github-api.port.js";
import type { RiskPolicy, AutopilotPolicy, ReleasePolicy } from "@/features/policy/index.js";
import type { RiskServices } from "@/features/risk/services.js";
import { CONTRACT_SCHEMA_VERSION } from "@/shared/domain/task/domain/contract/contract-types.js";
import type { Contract } from "@/types/contract.js";
import { mockContractStore } from "../../../../helpers/mocks.js";

// ─── Console capture ──────────────────────────────────────────────────────────

const originalConsoleLog = console.log;
const originalConsoleError = console.error;
const originalProcessExit = process.exit;

function captureConsole(): { logs: string[]; errors: string[] } {
  const logs: string[] = [];
  const errors: string[] = [];
  console.log = (...args: unknown[]) => { logs.push(args.map(String).join(" ")); };
  console.error = (...args: unknown[]) => { errors.push(args.map(String).join(" ")); };
  return { logs, errors };
}

afterEach(() => {
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
  process.exit = originalProcessExit;
});

// ─── Factories ─────────────────────────────────────────────────────────────────

function makeVerdict(decision: VerdictDecision = "PASS", overrides: Partial<Verdict> = {}): Verdict {
  return {
    schemaVersion: 1,
    id: generateVerdictId(),
    taskId: "tsk-aaaaaa",
    contractVersion: 1,
    computedAt: "2026-05-04T10:00:00.000Z",
    decision,
    effectiveRiskClass: "medium",
    proposedRiskClass: "medium",
    reasons: [{ category: "policy", code: "all-checks-passed", message: "All checks passed." }],
    evidenceConsulted: [],
    policiesConsulted: [],
    trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
    ...overrides,
  };
}

function makeContract(): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-000001",
    taskId: "tsk-aaaaaa",
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-01-01T00:00:00.000Z",
    intent: "Test",
    scope: { filesExpected: [], filesForbidden: [] },
    doneWhen: [],
    amendments: [],
    createdBy: "agent",
    configSnapshot: {
      strict: true,
      overlapPolicy: "fail",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
  };
}

function fakeVerdictStore(verdict: Verdict): VerdictStorePort {
  return {
    write: async () => {},
    readLatest: async () => verdict,
    readVersion: async () => verdict,
    history: async () => [verdict],
    findByTreeSha: async (treeSha) =>
      verdict.subject?.tree_sha === treeSha ? [verdict] : [],
    readLatestWithCorruption: async () => ({ verdict, corruptCount: 0 }),
  };
}

function fakeContractVersionStore(): ContractVersionStorePort {
  return {
    write: async () => {},
    readCurrent: async () => makeContract(),
    readVersion: async () => makeContract(),
    history: async () => [makeContract()],
  };
}

function fakeEvidenceStore(): EvidenceStorePort {
  return {
    append: async () => {},
    read: async () => undefined,
    list: async () => [],
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
    resolveTreeSha: async () => "deadbeef1234567890abcdef1234567890abcdef",
  };
}

function fakeRunStateStore(): RunStateStorePort {
  return {
    read: async () => undefined,
    write: async () => {},
    increment: async (_taskId, _delta) => ({
      schemaVersion: 1,
      taskId: _taskId,
      retryCount: 0,
      wallClockElapsedSeconds: 0,
      lastUpdatedAt: new Date().toISOString(),
    }),
  };
}

function makeRiskPolicy(): RiskPolicy {
  return { kind: "risk", id: "test", version: "1", rows: [] };
}

function makeAutopilotPolicy(): AutopilotPolicy {
  return {
    kind: "autopilot",
    id: "test",
    version: "1",
    autoMergeAllowed: { low: true, medium: true, high: false, critical: false },
    requiredWitnessLevel: {
      low: "agent-claimed-locally",
      medium: "agent-claimed-locally",
      high: "witnessed-by-maestro",
      critical: "witnessed-by-maestro",
    },
  };
}

function makeReleasePolicy(): ReleasePolicy {
  return { kind: "release", id: "test", version: "1", requireSignedCommits: false, requireProofMapComplete: false };
}

function fakeRiskServices(verdict: Verdict): RiskServices {
  return {
    computeRisk: () => verdict,
    deriveRiskClassFromDiff: () => ({ class: "medium", matchedRow: { signal: "diff-source-only" } }),
  };
}

interface FakeServices {
  verdictStore: VerdictStorePort;
  contractVersionStore: ContractVersionStorePort;
  contractStore: ContractStorePort;
  runStateStore: RunStateStorePort;
  legacyEvidenceStore: EvidenceStorePort;
  trustSpecStore: SpecStorePort;
  getEffectiveRiskPolicy: () => Promise<RiskPolicy>;
  getEffectiveAutopilotPolicy: () => Promise<AutopilotPolicy>;
  getEffectiveReleasePolicy: () => Promise<ReleasePolicy>;
  getEffectiveSensitivePathsGlobs: () => Promise<readonly string[]>;
  computeRisk: RiskServices["computeRisk"];
  deriveRiskClassFromDiff: RiskServices["deriveRiskClassFromDiff"];
  runTrustVerifier: (input: unknown) => Promise<{ findings: [] }>;
  gitAnchor: GitAnchorPort;
  githubApi: GithubApiPort;
  projectRoot: string;
}

function makeServices(verdict: Verdict): FakeServices {
  const riskServices = fakeRiskServices(verdict);
  return {
    verdictStore: fakeVerdictStore(verdict),
    contractVersionStore: fakeContractVersionStore(),
    contractStore: mockContractStore(),
    runStateStore: fakeRunStateStore(),
    legacyEvidenceStore: fakeEvidenceStore(),
    trustSpecStore: { read: async () => undefined, write: async () => {}, list: async () => [] },
    getEffectiveRiskPolicy: async () => makeRiskPolicy(),
    getEffectiveAutopilotPolicy: async () => makeAutopilotPolicy(),
    getEffectiveReleasePolicy: async () => makeReleasePolicy(),
    getEffectiveSensitivePathsGlobs: async () => [] as readonly string[],
    computeRisk: riskServices.computeRisk,
    deriveRiskClassFromDiff: riskServices.deriveRiskClassFromDiff,
    runTrustVerifier: async () => ({ findings: [] }),
    gitAnchor: fakeGitAnchor(),
    githubApi: {
      getPullRequestAuthor: async () => "test-user",
      postCheckRun: async () => ({ id: 1 }),
      patchCheckRun: async () => {},
      triggerAutoMerge: async () => {},
      listOpenPullRequests: async () => [],
      getPullRequestFiles: async () => [],
    },
    projectRoot: "/tmp/test-project",
  };
}

function makeProgram(services: FakeServices): Command {
  const program = new Command()
    .name("maestro")
    .option("--json")
    .exitOverride();
  const ciCmd = program
    .command("ci")
    .description("CI integration");
  registerCiVerifyCommand(ciCmd, program, { getServices: () => services });
  return program;
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("ci verify — exit codes", () => {
  const cases: VerdictDecision[] = ["PASS", "FAIL", "HUMAN", "BLOCK"];
  const expectedCodes = { PASS: 0, FAIL: 1, HUMAN: 2, BLOCK: 3 };

  for (const decision of cases) {
    it(`exits ${expectedCodes[decision]} for ${decision} decision`, async () => {
      const verdict = makeVerdict(decision);
      const services = makeServices(verdict);
      const program = makeProgram(services);
      captureConsole();

      let capturedExitCode: number | undefined;
      process.exit = (code?: number) => {
        capturedExitCode = code;
        throw new Error(`exit:${String(code)}`);
      };

      try {
        await program.parseAsync(["node", "maestro", "ci", "verify", "--task", "tsk-aaaaaa"]);
      } catch (err) {
        const msg = (err as Error).message;
        if (!msg.startsWith("exit:")) throw err;
      }

      expect(capturedExitCode).toBe(expectedCodes[decision] === 0 ? undefined : expectedCodes[decision]);
    });
  }
});

describe("ci verify — --json flag", () => {
  it("outputs parseable JSON with --json", async () => {
    const verdict = makeVerdict("PASS");
    const services = makeServices(verdict);
    const program = makeProgram(services);
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "ci", "verify", "--task", "tsk-aaaaaa", "--json"]);

    const parsed = JSON.parse(logs.join("")) as Verdict;
    expect(parsed.decision).toBe("PASS");
    expect(parsed.taskId).toBe("tsk-aaaaaa");
    expect(parsed.schemaVersion).toBe(1);
  });

  it("outputs human-readable text without --json", async () => {
    const verdict = makeVerdict("PASS");
    const services = makeServices(verdict);
    const program = makeProgram(services);
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "ci", "verify", "--task", "tsk-aaaaaa"]);

    const output = logs.join("\n");
    expect(output).toContain("Decision:");
    expect(output).toContain("PASS");
    expect(output).toContain("Task:");
  });
});

describe("ci verify — GITHUB_OUTPUT writing", () => {
  let tmpDir: string;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-ci-test-"));
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
    // Restore env state
    delete process.env.GITHUB_OUTPUT;
    delete process.env.GITHUB_ACTIONS;
    delete process.env.GITHUB_BASE_REF;
  });

  it("writes verdict_id, verdict_decision, effective_risk_class to $GITHUB_OUTPUT", async () => {
    const outputFile = join(tmpDir, "github-output");
    process.env.GITHUB_OUTPUT = outputFile;
    process.env.GITHUB_ACTIONS = "true";
    process.env.GITHUB_BASE_REF = "main";

    const verdict = makeVerdict("PASS");
    const services = makeServices(verdict);
    const program = makeProgram(services);
    captureConsole();

    await program.parseAsync(["node", "maestro", "ci", "verify", "--task", "tsk-aaaaaa"]);

    const outputContent = await readFile(outputFile, "utf8");
    expect(outputContent).toContain(`verdict_id=${verdict.id}`);
    expect(outputContent).toContain("verdict_decision=PASS");
    expect(outputContent).toContain("effective_risk_class=medium");
  });

  it("does not write GITHUB_OUTPUT when env var is not set", async () => {
    // Ensure GITHUB_OUTPUT is not set
    delete process.env.GITHUB_OUTPUT;

    const verdict = makeVerdict("PASS");
    const services = makeServices(verdict);
    const program = makeProgram(services);
    captureConsole();

    // Should not throw even without GITHUB_OUTPUT
    await program.parseAsync(["node", "maestro", "ci", "verify", "--task", "tsk-aaaaaa"]);
  });
});

describe("ci verify — flag parsing", () => {
  it("accepts --pr flag", async () => {
    const verdict = makeVerdict("PASS");
    const services = makeServices(verdict);
    const program = makeProgram(services);
    captureConsole();

    // Should not throw
    await program.parseAsync(["node", "maestro", "ci", "verify", "--task", "tsk-aaaaaa", "--pr", "42"]);
  });

  it("accepts --base flag", async () => {
    const verdict = makeVerdict("PASS");
    const services = makeServices(verdict);
    const program = makeProgram(services);
    captureConsole();

    await program.parseAsync(["node", "maestro", "ci", "verify", "--task", "tsk-aaaaaa", "--base", "origin/main"]);
  });
});
