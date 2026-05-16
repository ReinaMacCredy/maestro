import { afterEach, describe, expect, it } from "bun:test";
import { Command } from "commander";
import { registerPolicyCheckCommand } from "@/features/policy/commands/policy-check.command.js";
import type { RiskPolicy, AutopilotPolicy, ReleasePolicy } from "@/features/policy/index.js";
import type { ContractVersionStorePort, ContractStorePort, GitAnchorPort } from "@/shared/domain/legacy-task";
import type { RiskServices } from "@/features/risk/services.js";
import { CONTRACT_SCHEMA_VERSION } from "@/shared/domain/legacy-task/domain/contract/contract-types.js";
import type { Contract } from "@/types/contract.js";

// ─── Console capture ──────────────────────────────────────────────────────────

const originalConsoleLog = console.log;

function captureConsole(): { logs: string[] } {
  const logs: string[] = [];
  console.log = (...args: unknown[]) => { logs.push(args.map(String).join(" ")); };
  return { logs };
}

afterEach(() => {
  console.log = originalConsoleLog;
});

// ─── Factories ─────────────────────────────────────────────────────────────────

function makeContract(overrides: Partial<Contract> = {}): Contract {
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
    riskClass: "medium",
    ...overrides,
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

function fakeGitAnchor(changedPaths: string[] = []): GitAnchorPort {
  return {
    resolveRepoRoot: async (cwd) => cwd,
    resolveHeadCommit: async () => "abc1234",
    collectTouchedFiles: async () => ({ gitAvailable: true, actualFilesTouched: [] }),
    windowsOverlap: async () => false,
    collectChangedPaths: async () => changedPaths,
    collectAddedLines: async () => [],
    collectUntrackedFiles: async () => [],
    resolveTreeSha: async () => "tree123",
  };
}

function makeRiskPolicy(overrides: Partial<RiskPolicy> = {}): RiskPolicy {
  return {
    kind: "risk",
    id: "risk-policy-test",
    version: "1",
    rows: [
      { signal: "diff-source-only", derivedClass: "medium", description: "Source only" },
    ],
    ...overrides,
  };
}

function makeAutopilotPolicy(overrides: Partial<AutopilotPolicy> = {}): AutopilotPolicy {
  return {
    kind: "autopilot",
    id: "autopilot-policy-test",
    version: "1",
    autoMergeAllowed: { low: true, medium: true, high: false, critical: false },
    requiredWitnessLevel: {
      low: "agent-claimed-locally",
      medium: "agent-claimed-locally",
      high: "witnessed-by-maestro",
      critical: "witnessed-by-maestro",
    },
    ...overrides,
  };
}

function makeReleasePolicy(overrides: Partial<ReleasePolicy> = {}): ReleasePolicy {
  return {
    kind: "release",
    id: "release-policy-test",
    version: "1",
    requireSignedCommits: false,
    requireProofMapComplete: false,
    ...overrides,
  };
}

interface ServicesLike {
  contractVersionStore: ContractVersionStorePort;
  contractStore: ContractStorePort;
  getEffectiveRiskPolicy: () => Promise<RiskPolicy>;
  getEffectiveAutopilotPolicy: () => Promise<AutopilotPolicy>;
  getEffectiveReleasePolicy: () => Promise<ReleasePolicy>;
  deriveRiskClassFromDiff: RiskServices["deriveRiskClassFromDiff"];
  gitAnchor: GitAnchorPort;
  projectRoot: string;
}

function makeServices(overrides: Partial<ServicesLike> = {}): ServicesLike {
  return {
    contractVersionStore: fakeContractVersionStore(makeContract()),
    contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
    getEffectiveRiskPolicy: async () => makeRiskPolicy(),
    getEffectiveAutopilotPolicy: async () => makeAutopilotPolicy(),
    getEffectiveReleasePolicy: async () => makeReleasePolicy(),
    deriveRiskClassFromDiff: () => ({ class: "medium", matchedRow: { signal: "diff-source-only", description: "Source only" } }),
    gitAnchor: fakeGitAnchor(),
    projectRoot: "/tmp/test-project",
    ...overrides,
  };
}

function makeProgram(services: ServicesLike): Command {
  const program = new Command()
    .name("maestro")
    .option("--json")
    .exitOverride();
  const policyCmd = program.command("policy").description("Policy");
  registerPolicyCheckCommand(policyCmd, program, { getServices: () => services });
  return program;
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("policy check", () => {
  it("lists risk class and autopilot rules in text output", async () => {
    const services = makeServices();
    const program = makeProgram(services);
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "policy", "check", "--task", "tsk-aaaaaa"]);

    const output = logs.join("\n");
    expect(output).toContain("medium");
    expect(output).toContain("auto-merge allowed");
  });

  it("outputs valid JSON with --json flag", async () => {
    const services = makeServices();
    const program = makeProgram(services);
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "policy", "check", "--task", "tsk-aaaaaa", "--json"]);

    const parsed = JSON.parse(logs.join("")) as Record<string, unknown>;
    expect(parsed["taskId"]).toBe("tsk-aaaaaa");
    expect(parsed["effectiveRiskClass"]).toBe("medium");
    expect(parsed["autoMergeAllowed"]).toBe(true);
    expect(parsed["releaseRules"]).toBeDefined();
    expect(parsed["sensitivePaths"]).toBeDefined();
  });

  it("exits 0 always", async () => {
    const services = makeServices();
    const program = makeProgram(services);
    captureConsole();

    let exitCode: number | undefined;
    const original = process.exit;
    process.exit = (code?: number) => {
      exitCode = code;
      throw new Error(`exit:${String(code)}`);
    };
    try {
      await program.parseAsync(["node", "maestro", "policy", "check", "--task", "tsk-aaaaaa"]);
    } catch (err) {
      const msg = (err as Error).message;
      if (!msg.startsWith("exit:")) throw err;
    } finally {
      process.exit = original;
    }

    expect(exitCode).toBeUndefined();
  });

  it("shows release rules in text output", async () => {
    const services = makeServices({
      getEffectiveReleasePolicy: async () => makeReleasePolicy({ requireSignedCommits: true }),
    });
    const program = makeProgram(services);
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "policy", "check", "--task", "tsk-aaaaaa"]);

    const output = logs.join("\n");
    expect(output).toContain("require signed commits");
  });

  it("JSON output lists all policy sections", async () => {
    const services = makeServices({
      deriveRiskClassFromDiff: () => ({
        class: "high",
        matchedRow: { signal: "diff-modifies-ci-workflows", description: "CI workflows" },
      }),
    });
    const program = makeProgram(services);
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "policy", "check", "--task", "tsk-aaaaaa", "--json"]);

    const parsed = JSON.parse(logs.join("")) as Record<string, unknown>;
    expect(parsed["derivedRiskClass"]).toBe("high");
    expect(parsed["matchedRiskPolicyRow"]).toBeDefined();
    const row = parsed["matchedRiskPolicyRow"] as { signal: string };
    expect(row.signal).toBe("diff-modifies-ci-workflows");
  });
});
