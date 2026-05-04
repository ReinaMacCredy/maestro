import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { Command } from "commander";
import { registerVerdictCommand } from "@/features/verdict/commands/verdict.command.js";
import type { Verdict, VerdictDecision } from "@/features/verdict/domain/types.js";
import type { VerdictStorePort } from "@/features/verdict/ports/storage.js";
import { generateVerdictId } from "@/features/verdict/domain/verdict-id.js";
import type { ContractVersionStorePort } from "@/features/task/ports/contract-version-store.port.js";
import type { EvidenceStorePort } from "@/features/evidence/ports/storage.js";
import type { GitAnchorPort } from "@/features/task/ports/git-anchor.port.js";
import type { RiskPolicy, AutopilotPolicy, ReleasePolicy } from "@/features/policy/index.js";
import type { RiskServices } from "@/features/risk/services.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";
import type { Contract } from "@/features/task/index.js";

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
    policiesConsulted: [{ file: "policies/risk.yaml", version: "1" }],
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

function fakeVerdictStore(verdicts: Verdict[] = []): VerdictStorePort {
  const map = new Map(verdicts.map((v) => [v.id, v]));
  return {
    write: async (taskId, verdict) => { map.set(verdict.id, verdict); },
    readLatest: async () => {
      const all = [...map.values()].sort((a, b) => a.computedAt.localeCompare(b.computedAt));
      return all[all.length - 1];
    },
    readVersion: async (_taskId, id) => map.get(id),
    history: async () => [...map.values()].sort((a, b) => a.computedAt.localeCompare(b.computedAt)),
  };
}

function fakeContractVersionStore(contract?: Contract): ContractVersionStorePort {
  return {
    write: async () => {},
    readCurrent: async () => contract,
    readVersion: async () => contract,
    history: async () => (contract !== undefined ? [contract] : []),
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

interface ServicesLike {
  verdictStore: VerdictStorePort;
  contractVersionStore: ContractVersionStorePort;
  evidenceStore: EvidenceStorePort;
  getEffectiveRiskPolicy: () => Promise<RiskPolicy>;
  getEffectiveAutopilotPolicy: () => Promise<AutopilotPolicy>;
  getEffectiveReleasePolicy: () => Promise<ReleasePolicy>;
  computeRisk: RiskServices["computeRisk"];
  deriveRiskClassFromDiff: RiskServices["deriveRiskClassFromDiff"];
  runTrustVerifier: (input: unknown) => Promise<{ findings: [] }>;
  gitAnchor: GitAnchorPort;
  projectRoot: string;
}

function makeServices(verdict: Verdict, initialVerdicts: Verdict[] = []): ServicesLike {
  const riskServices = fakeRiskServices(verdict);
  return {
    verdictStore: fakeVerdictStore(initialVerdicts),
    contractVersionStore: fakeContractVersionStore(makeContract()),
    evidenceStore: fakeEvidenceStore(),
    getEffectiveRiskPolicy: async () => makeRiskPolicy(),
    getEffectiveAutopilotPolicy: async () => makeAutopilotPolicy(),
    getEffectiveReleasePolicy: async () => makeReleasePolicy(),
    computeRisk: riskServices.computeRisk,
    deriveRiskClassFromDiff: riskServices.deriveRiskClassFromDiff,
    runTrustVerifier: async () => ({ findings: [] }),
    gitAnchor: fakeGitAnchor(),
    projectRoot: "/tmp/test-project",
  };
}

function makeProgram(services: ServicesLike): Command {
  const program = new Command()
    .name("maestro")
    .option("--json")
    .exitOverride();
  registerVerdictCommand(program, { getServices: () => services });
  return program;
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("verdict show", () => {
  it("prints 'No verdict yet' and exits 0 when no verdicts exist", async () => {
    const verdict = makeVerdict("PASS");
    const services = makeServices(verdict, []);
    const program = makeProgram(services);
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "verdict", "show", "--task", "tsk-aaaaaa"]);

    expect(logs.join("\n")).toContain("No verdict yet");
  });

  it("prints decision and reasons after a verdict is stored", async () => {
    const verdict = makeVerdict("PASS");
    const services = makeServices(verdict, [verdict]);
    const program = makeProgram(services);
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "verdict", "show", "--task", "tsk-aaaaaa"]);

    const output = logs.join("\n");
    expect(output).toContain("PASS");
    expect(output).toContain("all-checks-passed");
  });

  it("outputs valid JSON with --json flag", async () => {
    const verdict = makeVerdict("FAIL");
    const services = makeServices(verdict, [verdict]);
    const program = makeProgram(services);
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "verdict", "show", "--task", "tsk-aaaaaa", "--json"]);

    const parsed = JSON.parse(logs.join("")) as Verdict;
    expect(parsed.decision).toBe("FAIL");
    expect(parsed.taskId).toBe("tsk-aaaaaa");
  });

  it("shows specific version by --version flag", async () => {
    const v1 = makeVerdict("PASS", { computedAt: "2026-05-04T08:00:00.000Z" });
    const v2 = makeVerdict("FAIL", { computedAt: "2026-05-04T10:00:00.000Z" });
    const services = makeServices(v2, [v1, v2]);
    const program = makeProgram(services);
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "verdict", "show", "--task", "tsk-aaaaaa", "--version", v1.id, "--json"]);

    const parsed = JSON.parse(logs.join("")) as Verdict;
    expect(parsed.id).toBe(v1.id);
    expect(parsed.decision).toBe("PASS");
  });
});

describe("verdict request", () => {
  it("computes a PASS verdict and exits 0", async () => {
    const verdict = makeVerdict("PASS");
    const services = makeServices(verdict);
    const program = makeProgram(services);
    const { logs } = captureConsole();

    let capturedExitCode: number | undefined;
    process.exit = (code?: number) => { capturedExitCode = code; throw new Error(`exit:${String(code)}`); };

    await program.parseAsync(["node", "maestro", "verdict", "request", "--task", "tsk-aaaaaa"]);

    const output = logs.join("\n");
    expect(output).toContain("PASS");
    expect(capturedExitCode).toBeUndefined();
  });

  it("exits 1 on FAIL decision", async () => {
    const verdict = makeVerdict("FAIL");
    const services = makeServices(verdict);
    const program = makeProgram(services);
    captureConsole();

    let capturedExitCode: number | undefined;
    process.exit = (code?: number) => {
      capturedExitCode = code;
      throw new Error(`exit:${String(code)}`);
    };

    try {
      await program.parseAsync(["node", "maestro", "verdict", "request", "--task", "tsk-aaaaaa"]);
    } catch (err) {
      const msg = (err as Error).message;
      if (!msg.startsWith("exit:")) throw err;
    }

    expect(capturedExitCode).toBe(1);
  });

  it("exits 2 on HUMAN decision", async () => {
    const verdict = makeVerdict("HUMAN");
    const services = makeServices(verdict);
    const program = makeProgram(services);
    captureConsole();

    let capturedExitCode: number | undefined;
    process.exit = (code?: number) => {
      capturedExitCode = code;
      throw new Error(`exit:${String(code)}`);
    };

    try {
      await program.parseAsync(["node", "maestro", "verdict", "request", "--task", "tsk-aaaaaa"]);
    } catch (err) {
      const msg = (err as Error).message;
      if (!msg.startsWith("exit:")) throw err;
    }

    expect(capturedExitCode).toBe(2);
  });

  it("exits 3 on BLOCK decision", async () => {
    const verdict = makeVerdict("BLOCK");
    const services = makeServices(verdict);
    const program = makeProgram(services);
    captureConsole();

    let capturedExitCode: number | undefined;
    process.exit = (code?: number) => {
      capturedExitCode = code;
      throw new Error(`exit:${String(code)}`);
    };

    try {
      await program.parseAsync(["node", "maestro", "verdict", "request", "--task", "tsk-aaaaaa"]);
    } catch (err) {
      const msg = (err as Error).message;
      if (!msg.startsWith("exit:")) throw err;
    }

    expect(capturedExitCode).toBe(3);
  });

  it("outputs valid JSON with --json flag", async () => {
    const verdict = makeVerdict("PASS");
    const services = makeServices(verdict);
    const program = makeProgram(services);
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "verdict", "request", "--task", "tsk-aaaaaa", "--json"]);

    const parsed = JSON.parse(logs.join("")) as Verdict;
    expect(parsed.decision).toBe("PASS");
    expect(parsed.taskId).toBe("tsk-aaaaaa");
  });
});
