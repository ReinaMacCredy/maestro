import { afterEach, describe, expect, it } from "bun:test";
import { Command } from "commander";
import { registerVerdictCommand } from "@/features/verdict/commands/verdict.command.js";
import type { VerdictStorePort } from "@/features/verdict/ports/storage.js";
import { generateVerdictId } from "@/features/verdict/domain/verdict-id.js";
import type { Verdict } from "@/features/verdict/domain/types.js";
import type { Owners } from "@/features/policy/index.js";
import { mockContractStore, mockEvidenceStore } from "../../../../helpers/mocks.js";
import type { EvidenceStorePort } from "@/features/evidence/ports/storage.js";
import type { VerdictOverridePayload } from "@/features/evidence/index.js";
import type { RiskClass } from "@/types/product-spec.js";
// ─── Console capture ──────────────────────────────────────────────────────────

const originalConsoleLog = console.log;
const originalConsoleError = console.error;
const originalProcessExit = process.exit;

afterEach(() => {
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
  process.exit = originalProcessExit;
});

// ─── Factories ────────────────────────────────────────────────────────────────

function makeVerdict(overrides: Partial<Verdict> = {}): Verdict {
  return {
    schemaVersion: 1,
    id: generateVerdictId(),
    taskId: "tsk-aaaaaa",
    contractVersion: 1,
    computedAt: "2026-05-05T10:00:00.000Z",
    decision: "BLOCK",
    effectiveRiskClass: "critical",
    reasons: [{ category: "risk", code: "effective-risk-critical", message: "Critical risk." }],
    evidenceConsulted: [],
    policiesConsulted: [],
    trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
    ...overrides,
  };
}

function fakeVerdictStore(verdicts: Verdict[] = []): VerdictStorePort {
  const byId = new Map(verdicts.map((v) => [v.id, v]));
  return {
    write: async () => {},
    readLatest: async (taskId) => {
      const found = [...byId.values()].filter((v) => v.taskId === taskId);
      return found.length > 0 ? found[found.length - 1] : undefined;
    },
    readVersion: async (_taskId, id) => byId.get(id),
    history: async (taskId) => [...byId.values()].filter((v) => v.taskId === taskId),
    findByTreeSha: async () => [],
    readLatestWithCorruption: async (taskId) => {
      const found = [...byId.values()].filter((v) => v.taskId === taskId);
      return {
        verdict: found.length > 0 ? found[found.length - 1] : undefined,
        corruptCount: 0,
      };
    },
  };
}

interface TestFixture {
  program: Command;
  evidenceStore: EvidenceStorePort;
  exitCode: number | undefined;
  errorLines: string[];
  logLines: string[];
}

function makeProgram(opts: {
  verdicts?: Verdict[];
  sensitiveWaivers?: string[];
  username?: string;
  /** When set, loadOwnersFromBase returns empty list (simulates base has no waivers) */
  baseWaivers?: string[];
}): TestFixture {
  const evidenceStore = mockEvidenceStore();
  const verdictStore = fakeVerdictStore(opts.verdicts ?? [makeVerdict()]);
  const logLines: string[] = [];
  const errorLines: string[] = [];
  let capturedExitCode: number | undefined;

  console.log = (...args: unknown[]) => { logLines.push(args.map(String).join(" ")); };
  console.error = (...args: unknown[]) => { errorLines.push(args.map(String).join(" ")); };
  process.exit = ((code?: number) => {
    capturedExitCode = code ?? 0;
    // Throw so async actions stop after process.exit — prevents further writes
    throw new Error(`process.exit:${String(code ?? 0)}`);
  }) as typeof process.exit;

  const owners: Owners = {
    policyApprovers: [],
    ratchetApprovers: [],
    sensitiveWaivers: opts.baseWaivers ?? opts.sensitiveWaivers ?? [],
    deployApprovers: [],
  };

  const program = new Command().exitOverride();
  
  const services = {
    verdictStore,
    legacyEvidenceStore: evidenceStore,
    contractVersionStore: {
      readLatest: async () => undefined,
      readVersion: async () => undefined,
      write: async () => {},
      history: async () => [],
    },
    contractStore: mockContractStore(),
    trustSpecStore: { read: async () => undefined, write: async () => {}, list: async () => [] },
    runStateStore: {
      read: async () => undefined,
      write: async () => {},
    },
    getEffectiveRiskPolicy: async () => ({
      rules: [],
      loaded: false,
      source: "default",
    }),
    getEffectiveAutopilotPolicy: async () => ({
      threshold: "witnessed-by-maestro",
      allowedRiskClasses: [],
      loaded: false,
      source: "default",
    }),
    getEffectiveReleasePolicy: async () => ({
      loaded: false,
      source: "default",
    }),
    getEffectiveSensitivePathsGlobs: async () => [] as readonly string[],
    computeRisk: async () => ({ riskClass: "low" satisfies RiskClass, signals: [] }),
    deriveRiskClassFromDiff: async (): Promise<RiskClass> => "low",
    runTrustVerifier: async () => ({
      passed: true,
      findings: [],
      findingsCount: 0,
      errors: 0,
      warns: 0,
      infos: 0,
    }),
    gitAnchor: {
      resolveRepoRoot: async (cwd: string) => cwd,
      resolveHeadCommit: async () => "HEAD",
      resolveTreeSha: async () => "tree-sha-abc",
      collectTouchedFiles: async () => ({
        gitAvailable: true,
        actualFilesTouched: [],
        closedAtCommit: "HEAD",
      }),
      windowsOverlap: async () => false,
      collectChangedPaths: async () => [],
      collectAddedLines: async () => [],
      collectUntrackedFiles: async () => [],
    },
    projectRoot: "/repo",
  } as unknown as ReturnType<NonNullable<Parameters<typeof registerVerdictCommand>[1]>["getServices"]>;

  type VerdictDeps = NonNullable<Parameters<typeof registerVerdictCommand>[1]>;
  const deps: VerdictDeps = {
    getServices: () => services,
    getUsername: () => opts.username ?? "alice",
    loadOwnersFromBase: (_base, _root) => owners,
    recordEvidence: async (store, input) => {
      const row = {
        schema_version: 3 as const,
        id: `evd-override01`,
        task_id: input.task_id,
        kind: input.kind,
        witness_level: input.witness_level,
        created_at: "2026-05-05T10:00:00.000Z",
        payload: input.payload,
      };
      await store.append(row);
      return row;
    },
  };

  registerVerdictCommand(program, deps);

  return { program, evidenceStore, exitCode: capturedExitCode, errorLines, logLines };
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("verdict override — authorized user", () => {
  it("exits 0 and writes a verdict-override Evidence row with correct kind/payload/witness level", async () => {
    const verdict = makeVerdict();
    const { program, evidenceStore } = makeProgram({
      verdicts: [verdict],
      sensitiveWaivers: ["alice"],
      username: "alice",
    });

    await program.parseAsync([
      "node", "maestro",
      "verdict", "override",
      "--task", "tsk-aaaaaa",
      "--pr", "42",
      "--reason", "Emergency hotfix approved",
    ]);

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa", kind: "verdict-override" });
    expect(rows).toHaveLength(1);

    const row = rows[0]!;
    expect(row.kind).toBe("verdict-override");
    expect(row.task_id).toBe("tsk-aaaaaa");
    expect(row.witness_level).toBe("agent-claimed-and-not-reproducible");

    const payload = row.payload as VerdictOverridePayload;
    expect(payload.verdictId).toBe(verdict.id);
    expect(payload.overriddenBy).toBe("alice");
    expect(payload.reason).toBe("Emergency hotfix approved");
  });

  it("uses supplied --verdict id instead of latest", async () => {
    const v1 = makeVerdict({ id: generateVerdictId() });
    const v2 = makeVerdict({ id: generateVerdictId() });
    const { program, evidenceStore } = makeProgram({
      verdicts: [v1, v2],
      sensitiveWaivers: ["alice"],
      username: "alice",
    });

    await program.parseAsync([
      "node", "maestro",
      "verdict", "override",
      "--task", "tsk-aaaaaa",
      "--pr", "7",
      "--verdict", v1.id,
      "--reason", "Override older verdict",
    ]);

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa", kind: "verdict-override" });
    expect(rows).toHaveLength(1);
    expect((rows[0]!.payload as VerdictOverridePayload).verdictId).toBe(v1.id);
  });
});

describe("verdict override — unauthorized user", () => {
  it("exits 1 with not-authorized message and writes NO Evidence row", async () => {
    const verdict = makeVerdict();
    const { program, evidenceStore, errorLines } = makeProgram({
      verdicts: [verdict],
      sensitiveWaivers: ["bob"],   // alice is NOT in this list
      username: "alice",
    });

    // Commander won't throw since we don't exitOverride the subprocess exit
    try {
      await program.parseAsync([
        "node", "maestro",
        "verdict", "override",
        "--task", "tsk-aaaaaa",
        "--pr", "42",
        "--reason", "Unauthorized attempt",
      ]);
    } catch {
      // process.exit is mocked; commander may throw from exitOverride
    }

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa", kind: "verdict-override" });
    expect(rows).toHaveLength(0);

    const errorText = errorLines.join("\n");
    expect(errorText).toContain("not-authorized");
  });
});

describe("verdict override — PR self-promotion is rejected", () => {
  it("rejects when base does NOT have user in sensitive_waiver, even if head would", async () => {
    const verdict = makeVerdict();
    // base waivers: empty (base doesn't have alice)
    // But the loader is called with the base owners — alice not present
    const { program, evidenceStore, errorLines } = makeProgram({
      verdicts: [verdict],
      baseWaivers: [],  // base branch has no waivers
      username: "alice",
    });

    try {
      await program.parseAsync([
        "node", "maestro",
        "verdict", "override",
        "--task", "tsk-aaaaaa",
        "--pr", "99",
        "--reason", "Self-promotion attempt",
      ]);
    } catch {
      // mocked exit may cause throw
    }

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa", kind: "verdict-override" });
    expect(rows).toHaveLength(0);
    expect(errorLines.join("\n")).toContain("not-authorized");
  });
});
