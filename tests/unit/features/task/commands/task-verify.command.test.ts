import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { Command } from "commander";
import { registerTaskVerifyCommand } from "@/features/task/commands/task-verify.command.js";
import { mockEvidenceStore, mockGitAnchor } from "../../../../helpers/mocks.js";
import { CONTRACT_SCHEMA_VERSION, type Contract } from "@/features/task/domain/contract/contract-types.js";
import type { EvidenceStorePort } from "@/features/evidence/index.js";
import type { GitAnchorPort } from "@/features/task/ports/git-anchor.port.js";
import type { ContractVersionStorePort } from "@/features/task/ports/contract-version-store.port.js";
import type { TrustFinding, TrustVerifierResult } from "@/features/verify/domain/types.js";

// ─── helpers ─────────────────────────────────────────────────────────────────

const TASK_ID = "tsk-abc123";

function makeBaseContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-abc123",
    taskId: TASK_ID,
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-05-01T00:00:00.000Z",
    intent: "implement feature",
    scope: {
      filesExpected: ["src/**"],
      filesForbidden: [],
    },
    doneWhen: [],
    amendments: [],
    createdBy: "session:codex:1",
    configSnapshot: {
      strict: false,
      overlapPolicy: "fail",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    ...overrides,
  };
}

function mockContractVersionStore(contract: Contract | undefined): ContractVersionStorePort {
  return {
    write: async () => {},
    readCurrent: async (_taskId: string) => contract,
    readVersion: async () => undefined,
    history: async () => (contract ? [contract] : []),
  };
}

function mockTrustVerifier(findings: readonly TrustFinding[]) {
  return async (_input: unknown): Promise<TrustVerifierResult> => ({ findings });
}

function makeProgram(): Command {
  return new Command().name("maestro").option("--json", "Output as JSON").exitOverride();
}

interface TestDeps {
  readonly contractVersionStore: ContractVersionStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly gitAnchor: GitAnchorPort;
  readonly runTrustVerifier: (input: unknown) => Promise<TrustVerifierResult>;
}

function makeDeps(opts: Partial<TestDeps> = {}): TestDeps {
  return {
    contractVersionStore: opts.contractVersionStore ?? mockContractVersionStore(makeBaseContract()),
    evidenceStore: opts.evidenceStore ?? mockEvidenceStore(),
    gitAnchor: opts.gitAnchor ?? mockGitAnchor({
      collectChangedPaths: async () => ["src/feature.ts"],
      collectAddedLines: async () => ["+const x = 1;"],
      collectUntrackedFiles: async () => [],
    }),
    runTrustVerifier: opts.runTrustVerifier ?? mockTrustVerifier([]),
  };
}

// ─── console capture ─────────────────────────────────────────────────────────

const originalConsoleLog = console.log;
const originalConsoleError = console.error;

function captureConsole(): { logs: string[]; errors: string[] } {
  const logs: string[] = [];
  const errors: string[] = [];
  console.log = (...args: unknown[]) => logs.push(args.map(String).join(" "));
  console.error = (...args: unknown[]) => errors.push(args.map(String).join(" "));
  return { logs, errors };
}

afterEach(() => {
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
});

// ─── process.exit capture ────────────────────────────────────────────────────

let capturedExitCode: number | undefined;
const originalProcessExit = process.exit;

beforeEach(() => {
  capturedExitCode = undefined;
  process.exit = ((code?: number) => {
    capturedExitCode = code ?? 0;
    throw new Error(`process.exit(${code})`);
  }) as typeof process.exit;
});

afterEach(() => {
  process.exit = originalProcessExit;
});

// ─── test runner helper ──────────────────────────────────────────────────────

async function runVerify(
  argv: string[],
  deps: TestDeps,
): Promise<{ logs: string[]; errors: string[]; exitCode: number }> {
  const program = makeProgram();
  const taskCmd = program.command("task");
  registerTaskVerifyCommand(taskCmd, program, {
    getServices: () => deps,
  });

  const { logs, errors } = captureConsole();

  try {
    await program.parseAsync(["node", "maestro", "task", "verify", ...argv]);
    return { logs, errors, exitCode: capturedExitCode ?? 0 };
  } catch (err) {
    if (err instanceof Error && err.message.startsWith("process.exit(")) {
      return { logs, errors, exitCode: capturedExitCode ?? 0 };
    }
    throw err;
  }
}

// ─── tests ───────────────────────────────────────────────────────────────────

describe("task verify", () => {
  describe("clean diff — exit 0", () => {
    it("prints 'no findings' and exits 0", async () => {
      const deps = makeDeps({ runTrustVerifier: mockTrustVerifier([]) });
      const { logs, exitCode } = await runVerify(["--task", TASK_ID, "--base", "abc123"], deps);
      expect(logs.some((l) => l.includes("no findings"))).toBe(true);
      expect(exitCode).toBe(0);
    });

    it("writes no evidence rows", async () => {
      const evidenceStore = mockEvidenceStore();
      const deps = makeDeps({ evidenceStore, runTrustVerifier: mockTrustVerifier([]) });
      await runVerify(["--task", TASK_ID, "--base", "abc123"], deps);
      const rows = await evidenceStore.list({ task_id: TASK_ID });
      expect(rows).toHaveLength(0);
    });
  });

  describe("forbidden path violation — exit 1", () => {
    const forbiddenFinding: TrustFinding = {
      check: "scope",
      severity: "error",
      paths: ["config/forbidden.ts"],
      details: "Path is forbidden by contract scope",
    };

    it("exits 1", async () => {
      const deps = makeDeps({ runTrustVerifier: mockTrustVerifier([forbiddenFinding]) });
      const { exitCode } = await runVerify(["--task", TASK_ID, "--base", "abc123"], deps);
      expect(exitCode).toBe(1);
    });

    it("lists the finding", async () => {
      const deps = makeDeps({ runTrustVerifier: mockTrustVerifier([forbiddenFinding]) });
      const { logs } = await runVerify(["--task", TASK_ID, "--base", "abc123"], deps);
      expect(logs.some((l) => l.includes("[error]"))).toBe(true);
      expect(logs.some((l) => l.includes("scope"))).toBe(true);
    });

    it("writes 1 verifier-kind evidence row", async () => {
      const evidenceStore = mockEvidenceStore();
      const deps = makeDeps({ evidenceStore, runTrustVerifier: mockTrustVerifier([forbiddenFinding]) });
      await runVerify(["--task", TASK_ID, "--base", "abc123"], deps);
      const rows = await evidenceStore.list({ task_id: TASK_ID, kind: "verifier" });
      expect(rows).toHaveLength(1);
      expect(rows[0]!.witness_level).toBe("agent-claimed-locally");
      expect(rows[0]!.payload).toMatchObject({
        check: "scope",
        severity: "error",
        paths: ["config/forbidden.ts"],
      });
    });
  });

  describe("sensitive path warning — exit 2", () => {
    const sensitiveFinding: TrustFinding = {
      check: "sensitive-paths",
      severity: "warn",
      paths: [".maestro/policies/sensitive-paths.yaml"],
    };

    it("exits 2", async () => {
      const deps = makeDeps({ runTrustVerifier: mockTrustVerifier([sensitiveFinding]) });
      const { exitCode } = await runVerify(["--task", TASK_ID, "--base", "abc123"], deps);
      expect(exitCode).toBe(2);
    });

    it("writes 1 evidence row", async () => {
      const evidenceStore = mockEvidenceStore();
      const deps = makeDeps({ evidenceStore, runTrustVerifier: mockTrustVerifier([sensitiveFinding]) });
      await runVerify(["--task", TASK_ID, "--base", "abc123"], deps);
      const rows = await evidenceStore.list({ task_id: TASK_ID, kind: "verifier" });
      expect(rows).toHaveLength(1);
      expect(rows[0]!.payload).toMatchObject({ severity: "warn" });
    });
  });

  describe("info-only findings exit 0", () => {
    const infoFinding: TrustFinding = {
      check: "commit-metadata",
      severity: "info",
      paths: [],
      details: "Unsigned commit (info-only)",
    };

    it("exits 0 when only info-level findings are present", async () => {
      const deps = makeDeps({ runTrustVerifier: mockTrustVerifier([infoFinding]) });
      const { exitCode } = await runVerify(["--task", TASK_ID, "--base", "abc123"], deps);
      expect(exitCode).toBe(0);
    });
  });

  describe("untracked out-of-scope files — exit 2", () => {
    it("emits warn-level finding when untracked files are out of scope", async () => {
      const contract = makeBaseContract({ scope: { filesExpected: ["src/**"], filesForbidden: [] } });
      const deps = makeDeps({
        contractVersionStore: mockContractVersionStore(contract),
        gitAnchor: mockGitAnchor({
          collectChangedPaths: async () => [],
          collectAddedLines: async () => [],
          collectUntrackedFiles: async () => ["dist/build.js", "temp.log"],
        }),
        runTrustVerifier: mockTrustVerifier([]),
      });
      const { logs, exitCode } = await runVerify(["--task", TASK_ID, "--base", "abc123"], deps);
      expect(exitCode).toBe(2);
      expect(logs.some((l) => l.includes("[warn]"))).toBe(true);
      expect(logs.some((l) => l.includes("untracked-out-of-scope"))).toBe(true);
    });

    it("writes evidence row for untracked-out-of-scope finding", async () => {
      const contract = makeBaseContract({ scope: { filesExpected: ["src/**"], filesForbidden: [] } });
      const evidenceStore = mockEvidenceStore();
      const deps = makeDeps({
        contractVersionStore: mockContractVersionStore(contract),
        evidenceStore,
        gitAnchor: mockGitAnchor({
          collectChangedPaths: async () => [],
          collectAddedLines: async () => [],
          collectUntrackedFiles: async () => ["dist/build.js"],
        }),
        runTrustVerifier: mockTrustVerifier([]),
      });
      await runVerify(["--task", TASK_ID, "--base", "abc123"], deps);
      const rows = await evidenceStore.list({ task_id: TASK_ID, kind: "verifier" });
      expect(rows).toHaveLength(1);
      expect(rows[0]!.payload).toMatchObject({
        check: "untracked-out-of-scope",
        severity: "warn",
        paths: ["dist/build.js"],
      });
    });

    it("does not emit finding when untracked files are in scope", async () => {
      const contract = makeBaseContract({ scope: { filesExpected: ["src/**"], filesForbidden: [] } });
      const deps = makeDeps({
        contractVersionStore: mockContractVersionStore(contract),
        gitAnchor: mockGitAnchor({
          collectChangedPaths: async () => [],
          collectAddedLines: async () => [],
          collectUntrackedFiles: async () => ["src/new-feature.ts"],
        }),
        runTrustVerifier: mockTrustVerifier([]),
      });
      const { logs, exitCode } = await runVerify(["--task", TASK_ID, "--base", "abc123"], deps);
      expect(exitCode).toBe(0);
      expect(logs.some((l) => l.includes("no findings"))).toBe(true);
    });
  });

  describe("--json output", () => {
    const finding: TrustFinding = {
      check: "scope",
      severity: "error",
      paths: ["src/bad.ts"],
    };

    it("outputs parseable JSON with findings and counts", async () => {
      const deps = makeDeps({ runTrustVerifier: mockTrustVerifier([finding]) });
      const program = new Command().name("maestro").option("--json", "Output as JSON").exitOverride();
      const taskCmd = program.command("task");
      registerTaskVerifyCommand(taskCmd, program, { getServices: () => deps });

      const written: string[] = [];
      const originalWrite = process.stdout.write.bind(process.stdout);
      process.stdout.write = ((chunk: unknown) => {
        if (typeof chunk === "string") written.push(chunk);
        return true;
      }) as typeof process.stdout.write;

      captureConsole();
      try {
        await program.parseAsync(["node", "maestro", "task", "verify", "--task", TASK_ID, "--base", "abc123", "--json"]);
      } catch {
        // process.exit throws
      } finally {
        process.stdout.write = originalWrite;
      }

      const raw = written.join("");
      const parsed = JSON.parse(raw) as { findings: TrustFinding[]; counts: Record<string, number> };
      expect(parsed.findings).toHaveLength(1);
      expect(parsed.findings[0]!.check).toBe("scope");
      expect(parsed.counts).toMatchObject({ error: 1, warn: 0, info: 0 });
    });
  });

  describe("no contract proposed", () => {
    it("throws MaestroError with /no contract/i", async () => {
      const deps = makeDeps({
        contractVersionStore: mockContractVersionStore(undefined),
      });
      const program = makeProgram();
      const taskCmd = program.command("task");
      registerTaskVerifyCommand(taskCmd, program, { getServices: () => deps });

      captureConsole();
      await expect(
        program.parseAsync(["node", "maestro", "task", "verify", "--task", TASK_ID, "--base", "abc123"]),
      ).rejects.toThrow(/no contract/i);
    });
  });
});
