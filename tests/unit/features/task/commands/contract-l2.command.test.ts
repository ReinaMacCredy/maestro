import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Command } from "commander";
import { FsContractVersionStoreAdapter } from "@/features/task/adapters/fs-contract-version-store.adapter.js";
import { CONTRACT_SCHEMA_VERSION, type AmendmentBudget, type Contract, type ContractAmendment } from "@/features/task/domain/contract/contract-types.js";
import { proposeContract } from "@/features/task/usecases/propose-contract.usecase.js";
import { amendContract } from "@/features/task/usecases/amend-contract.usecase.js";
import { registerContractL2Command } from "@/features/task/commands/contract-l2.command.js";
import { mockEvidenceStore, mockContractStore } from "../../../../helpers/mocks.js";
import type { EvidenceStorePort } from "@/features/evidence/index.js";
import type { ContractVersionStorePort } from "@/features/task/ports/contract-version-store.port.js";
import type { ContractStorePort } from "@/features/task/ports/contract-store.port.js";

// ─── helpers ─────────────────────────────────────────────────────────────────

const TASK_ID = "tsk-a1b2c3";

function makeBaseContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-a1b2c3",
    taskId: TASK_ID,
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-05-01T00:00:00.000Z",
    intent: "implement feature",
    scope: {
      filesExpected: ["src/feature.ts"],
      filesForbidden: [],
    },
    doneWhen: [],
    amendments: [],
    createdBy: "session:codex:1",
    configSnapshot: {
      strict: false,
      overlapPolicy: "fail" as const,
      rebaseFallback: "best-effort" as const,
      staleReclaimContractPolicy: "inherit" as const,
    },
    riskClass: "medium",
    ...overrides,
  };
}

function makeAmendment(hex: string, reason: string): ContractAmendment {
  // Amendment IDs must match /^a-[0-9a-f]{6}$/ per AMENDMENT_ID_PATTERN
  return {
    id: `a-${hex}`,
    at: "2026-05-01T01:00:00.000Z",
    by: "session:codex:1",
    reason,
    before: { intent: "old" },
    after: { intent: "new" },
  };
}

function makeProgram(): Command {
  return new Command().name("maestro").option("--json", "Output as JSON").exitOverride();
}

interface TestDeps {
  readonly store: FsContractVersionStoreAdapter;
  readonly evidenceStore: EvidenceStorePort;
  readonly contractStore: ContractStorePort;
}

function makeDeps(store: ContractVersionStorePort, evidenceStore: EvidenceStorePort, contractStore: ContractStorePort) {
  return {
    getServices: () => ({ contractVersionStore: store, evidenceStore, contractStore }),
    amendContract,
  };
}

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

// ─── test setup ──────────────────────────────────────────────────────────────

let tmpDir: string;
let deps: TestDeps;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "contract-l2-cmd-"));
  const store = new FsContractVersionStoreAdapter(tmpDir);
  const evidenceStore = mockEvidenceStore();
  const contractStore = mockContractStore();
  deps = { store, evidenceStore, contractStore };

  // Seed: propose v1
  const budget: AmendmentBudget = {
    maxAmendments: 5,
    maxPathsPerAmendment: 10,
    forbiddenAmendmentPaths: [],
  };
  await proposeContract(store, makeBaseContract({ amendmentBudget: budget }));

  // Amend → v2
  await amendContract(store, evidenceStore, {
    taskId: TASK_ID,
    amendment: makeAmendment("000001", "first amendment"),
    addedPaths: ["src/helper.ts"],
    removedPaths: [],
  });

  // Amend → v3
  await amendContract(store, evidenceStore, {
    taskId: TASK_ID,
    amendment: makeAmendment("000002", "second amendment"),
    addedPaths: ["src/utils.ts"],
    removedPaths: [],
  });
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

// ─── show ─────────────────────────────────────────────────────────────────────

describe("contract show", () => {
  it("defaults to current version (v3)", async () => {
    const { logs } = captureConsole();
    const program = makeProgram();
    registerContractL2Command(program, makeDeps(deps.store, deps.evidenceStore, deps.contractStore));

    await program.parseAsync(["node", "maestro", "contract", "show", "--task", TASK_ID]);

    const joined = logs.join("\n");
    // v3 has 2 amendments
    expect(joined).toContain("Amendments (2)");
  });

  it("shows v1 when --at-version 1", async () => {
    const { logs } = captureConsole();
    const program = makeProgram();
    registerContractL2Command(program, makeDeps(deps.store, deps.evidenceStore, deps.contractStore));

    await program.parseAsync(["node", "maestro", "contract", "show", "--task", TASK_ID, "--at-version", "1"]);

    const joined = logs.join("\n");
    // v1 has no amendments
    expect(joined).toContain("(none)");
  });

  it("exits non-zero and prints clear error for --at-version 999", async () => {
    captureConsole();
    const program = makeProgram();
    registerContractL2Command(program, makeDeps(deps.store, deps.evidenceStore, deps.contractStore));

    let thrown: unknown;
    try {
      await program.parseAsync(["node", "maestro", "contract", "show", "--task", TASK_ID, "--at-version", "999"]);
    } catch (err) {
      thrown = err;
    }

    expect(thrown).toBeDefined();
    const msg = (thrown as Error).message;
    expect(msg).toMatch(/999/);
    expect(msg).toMatch(/does not exist/);
  });

  it("returns valid JSON with --json", async () => {
    const { logs } = captureConsole();
    const program = makeProgram();
    registerContractL2Command(program, makeDeps(deps.store, deps.evidenceStore, deps.contractStore));

    await program.parseAsync(["node", "maestro", "contract", "show", "--task", TASK_ID, "--json"]);

    const parsed = JSON.parse(logs.join("\n"));
    expect(parsed).toHaveProperty("taskId", TASK_ID);
    expect(parsed).toHaveProperty("status");
  });

  it("returns specific version JSON with --at-version 1 --json", async () => {
    const { logs } = captureConsole();
    const program = makeProgram();
    registerContractL2Command(program, makeDeps(deps.store, deps.evidenceStore, deps.contractStore));

    await program.parseAsync(["node", "maestro", "contract", "show", "--task", TASK_ID, "--at-version", "1", "--json"]);

    const parsed = JSON.parse(logs.join("\n"));
    expect(parsed).toHaveProperty("taskId", TASK_ID);
    expect(parsed.amendments).toHaveLength(0);
  });
});

// ─── amend ────────────────────────────────────────────────────────────────────

describe("contract amend", () => {
  it("succeeds within budget and prints new version", async () => {
    const { logs } = captureConsole();
    const program = makeProgram();
    registerContractL2Command(program, makeDeps(deps.store, deps.evidenceStore, deps.contractStore));

    await program.parseAsync([
      "node", "maestro", "contract", "amend",
      "--task", TASK_ID,
      "--add-path", "src/new-service.ts",
      "--reason", "adding new service",
    ]);

    const joined = logs.join("\n");
    expect(joined).toContain("Contract amended");
    expect(joined).toMatch(/New version:\s*4/);
  });

  it("exits non-zero matching /budget/i when amendment count exceeds maxAmendments", async () => {
    // Exhaust the remaining 3 amendments (already used 2 in setup)
    const extraHexes = ["00000a", "00000b", "00000c"];
    for (let i = 0; i < 3; i++) {
      await amendContract(deps.store, deps.evidenceStore, {
        taskId: TASK_ID,
        amendment: makeAmendment(extraHexes[i]!, `extra ${i + 3}`),
        addedPaths: [`src/extra${i + 3}.ts`],
        removedPaths: [],
      });
    }

    captureConsole();
    const program = makeProgram();
    registerContractL2Command(program, makeDeps(deps.store, deps.evidenceStore, deps.contractStore));

    let thrown: unknown;
    try {
      await program.parseAsync([
        "node", "maestro", "contract", "amend",
        "--task", TASK_ID,
        "--add-path", "src/over-budget.ts",
        "--reason", "should fail",
      ]);
    } catch (err) {
      thrown = err;
    }

    expect(thrown).toBeDefined();
    expect((thrown as Error).message).toMatch(/budget/i);
  });

  it("exits non-zero matching /forbidden/i when path matches forbiddenAmendmentPaths", async () => {
    // Re-propose with forbidden patterns
    const forbiddenStore = new FsContractVersionStoreAdapter(
      await mkdtemp(join(tmpdir(), "contract-l2-forbidden-")),
    );
    const budget: AmendmentBudget = {
      maxAmendments: 10,
      maxPathsPerAmendment: 10,
      forbiddenAmendmentPaths: ["secrets/**"],
    };
    await proposeContract(forbiddenStore, makeBaseContract({ taskId: "tsk-b2c3d4", amendmentBudget: budget }));

    captureConsole();
    const program = makeProgram();
    registerContractL2Command(program, makeDeps(forbiddenStore, deps.evidenceStore, deps.contractStore));

    let thrown: unknown;
    try {
      await program.parseAsync([
        "node", "maestro", "contract", "amend",
        "--task", "tsk-b2c3d4",
        "--add-path", "secrets/key.txt",
        "--reason", "should fail",
      ]);
    } catch (err) {
      thrown = err;
    }

    expect(thrown).toBeDefined();
    expect((thrown as Error).message).toMatch(/forbidden/i);
  });

  it("returns valid JSON with --json on success", async () => {
    const { logs } = captureConsole();
    const program = makeProgram();
    registerContractL2Command(program, makeDeps(deps.store, deps.evidenceStore, deps.contractStore));

    await program.parseAsync([
      "node", "maestro", "contract", "amend",
      "--task", TASK_ID,
      "--add-path", "src/another.ts",
      "--reason", "json test",
      "--json",
    ]);

    const parsed = JSON.parse(logs.join("\n"));
    expect(parsed).toHaveProperty("amendmentId");
    expect(parsed).toHaveProperty("newVersion");
    expect(typeof parsed.newVersion).toBe("number");
  });
});

// ─── history ──────────────────────────────────────────────────────────────────

describe("contract history", () => {
  it("returns 3 versions in ascending order (v1, v2, v3)", async () => {
    const { logs } = captureConsole();
    const program = makeProgram();
    registerContractL2Command(program, makeDeps(deps.store, deps.evidenceStore, deps.contractStore));

    await program.parseAsync(["node", "maestro", "contract", "history", "--task", TASK_ID]);

    const joined = logs.join("\n");
    expect(joined).toContain("v1");
    expect(joined).toContain("v2");
    expect(joined).toContain("v3");
    // ascending: v1 appears before v2, v2 before v3
    expect(joined.indexOf("v1")).toBeLessThan(joined.indexOf("v2"));
    expect(joined.indexOf("v2")).toBeLessThan(joined.indexOf("v3"));
  });

  it("returns valid JSON array with --json", async () => {
    const { logs } = captureConsole();
    const program = makeProgram();
    registerContractL2Command(program, makeDeps(deps.store, deps.evidenceStore, deps.contractStore));

    await program.parseAsync(["node", "maestro", "contract", "history", "--task", TASK_ID, "--json"]);

    const parsed = JSON.parse(logs.join("\n"));
    expect(Array.isArray(parsed)).toBe(true);
    expect(parsed).toHaveLength(3);
    // ascending order check via createdAt or position
    expect(parsed[0].amendments).toHaveLength(0);    // v1: 0 amendments
    expect(parsed[1].amendments).toHaveLength(1);    // v2: 1 amendment
    expect(parsed[2].amendments).toHaveLength(2);    // v3: 2 amendments
  });
});
