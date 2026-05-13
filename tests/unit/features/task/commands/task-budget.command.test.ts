import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { Command } from "commander";
import { registerTaskBudgetCommand } from "@/features/task/commands/task-budget.command.js";
import type { ContractVersionStorePort } from "@/features/task/ports/contract-version-store.port.js";
import type { RunStateStorePort } from "@/features/task/ports/run-state-store.port.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";
import type { RunState } from "@/features/task/domain/run-state.js";

// ─── Factories ─────────────────────────────────────────────────────────────────

function makeContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-000001",
    taskId: "tsk-aaaaaa",
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-01-01T00:00:00.000Z",
    intent: "Test task",
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
    costBudget: { maxRetries: 5, maxWallClockSeconds: 3600, maxTokens: 100_000 },
    ...overrides,
  };
}

function makeRunState(overrides: Partial<RunState> = {}): RunState {
  return {
    schemaVersion: 1,
    taskId: "tsk-aaaaaa",
    retryCount: 2,
    wallClockElapsedSeconds: 120,
    lastUpdatedAt: "2026-01-01T00:00:00.000Z",
    ...overrides,
  };
}

function fakeContractVersionStore(contract: Contract | undefined): ContractVersionStorePort {
  return {
    write: async () => {},
    readCurrent: async () => contract,
    readVersion: async () => contract,
    history: async () => (contract ? [contract] : []),
  };
}

function fakeRunStateStore(state: RunState | undefined): RunStateStorePort {
  return {
    read: async () => state,
    write: async () => {},
    increment: async () => state ?? { schemaVersion: 1, taskId: "tsk-aaaaaa", retryCount: 0, wallClockElapsedSeconds: 0, lastUpdatedAt: "2026-01-01T00:00:00.000Z" },
  };
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("registerTaskBudgetCommand", () => {
  let originalLog: typeof console.log;
  let originalStdoutWrite: typeof process.stdout.write;

  beforeEach(() => {
    originalLog = console.log;
    originalStdoutWrite = process.stdout.write.bind(process.stdout);
  });

  afterEach(() => {
    console.log = originalLog;
    process.stdout.write = originalStdoutWrite;
  });

  it("prints 'No contract for task' when contract is missing", async () => {
    const lines: string[] = [];
    console.log = (...args: unknown[]) => lines.push(args.map(String).join(" "));

    const program = new Command();
    program.exitOverride();
    const taskCmd = program.command("task");
    registerTaskBudgetCommand(taskCmd, program, {
      getServices: () => ({
        contractVersionStore: fakeContractVersionStore(undefined),
        runStateStore: fakeRunStateStore(undefined),
        contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
      }),
    });

    await program.parseAsync(["task", "budget", "--task", "tsk-aaaaaa"], { from: "user" });
    expect(lines.join("\n")).toContain("No contract for task tsk-aaaaaa");
  });

  it("text mode lists retries, wall clock, and tokens with current/max format", async () => {
    const lines: string[] = [];
    console.log = (...args: unknown[]) => lines.push(args.map(String).join(" "));

    const contract = makeContract({ costBudget: { maxRetries: 5, maxWallClockSeconds: 3600, maxTokens: 100_000 } });
    const state = makeRunState({ retryCount: 2, wallClockElapsedSeconds: 120, tokensUsed: 4500 });

    const program = new Command();
    program.exitOverride();
    const taskCmd = program.command("task");
    registerTaskBudgetCommand(taskCmd, program, {
      getServices: () => ({
        contractVersionStore: fakeContractVersionStore(contract),
        runStateStore: fakeRunStateStore(state),
        contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
      }),
    });

    await program.parseAsync(["task", "budget", "--task", "tsk-aaaaaa"], { from: "user" });
    const joined = lines.join("\n");
    expect(joined).toContain("2/5");
    expect(joined).toContain("120s/3600s");
    expect(joined).toContain("4500/100000");
    expect(joined).toContain("Exhausted:  no");
  });

  it("text mode shows Exhausted: yes with reason when budget is exhausted", async () => {
    const lines: string[] = [];
    console.log = (...args: unknown[]) => lines.push(args.map(String).join(" "));

    const contract = makeContract({ costBudget: { maxRetries: 5, maxWallClockSeconds: 3600 } });
    const state = makeRunState({ retryCount: 5, wallClockElapsedSeconds: 0 });

    const program = new Command();
    program.exitOverride();
    const taskCmd = program.command("task");
    registerTaskBudgetCommand(taskCmd, program, {
      getServices: () => ({
        contractVersionStore: fakeContractVersionStore(contract),
        runStateStore: fakeRunStateStore(state),
        contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
      }),
    });

    await program.parseAsync(["task", "budget", "--task", "tsk-aaaaaa"], { from: "user" });
    const joined = lines.join("\n");
    expect(joined).toContain("Exhausted:  yes (max-retries)");
  });

  it("JSON mode outputs parseable JSON with all expected keys", async () => {
    let captured = "";
    const originalWrite = process.stdout.write.bind(process.stdout);
    process.stdout.write = (data: string | Uint8Array, ...rest: unknown[]) => {
      if (typeof data === "string") captured += data;
      return true;
    };

    const contract = makeContract({ costBudget: { maxRetries: 5, maxWallClockSeconds: 3600, maxTokens: 100_000 } });
    const state = makeRunState({ retryCount: 2, wallClockElapsedSeconds: 120, tokensUsed: 4500 });

    const program = new Command();
    program.exitOverride();
    const taskCmd = program.command("task");
    registerTaskBudgetCommand(taskCmd, program, {
      getServices: () => ({
        contractVersionStore: fakeContractVersionStore(contract),
        runStateStore: fakeRunStateStore(state),
        contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
      }),
    });

    await program.parseAsync(["task", "budget", "--task", "tsk-aaaaaa", "--json"], { from: "user" });
    process.stdout.write = originalWrite;

    const parsed = JSON.parse(captured);
    expect(parsed.taskId).toBe("tsk-aaaaaa");
    expect(parsed.retryCount).toBe(2);
    expect(parsed.maxRetries).toBe(5);
    expect(parsed.wallClockElapsedSeconds).toBe(120);
    expect(parsed.maxWallClockSeconds).toBe(3600);
    expect(parsed.tokensUsed).toBe(4500);
    expect(parsed.maxTokens).toBe(100_000);
    expect(parsed.exhausted).toBe(false);
  });

  it("JSON mode includes reason when exhausted", async () => {
    let captured = "";
    const originalWrite = process.stdout.write.bind(process.stdout);
    process.stdout.write = (data: string | Uint8Array, ...rest: unknown[]) => {
      if (typeof data === "string") captured += data;
      return true;
    };

    const contract = makeContract({ costBudget: { maxRetries: 5, maxWallClockSeconds: 3600 } });
    const state = makeRunState({ retryCount: 5, wallClockElapsedSeconds: 0 });

    const program = new Command();
    program.exitOverride();
    const taskCmd = program.command("task");
    registerTaskBudgetCommand(taskCmd, program, {
      getServices: () => ({
        contractVersionStore: fakeContractVersionStore(contract),
        runStateStore: fakeRunStateStore(state),
        contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
      }),
    });

    await program.parseAsync(["task", "budget", "--task", "tsk-aaaaaa", "--json"], { from: "user" });
    process.stdout.write = originalWrite;

    const parsed = JSON.parse(captured);
    expect(parsed.exhausted).toBe(true);
    expect(parsed.reason).toBe("max-retries");
  });

  it("text mode marks contract as having no budget when costBudget is undefined", async () => {
    const lines: string[] = [];
    console.log = (...args: unknown[]) => lines.push(args.map(String).join(" "));

    // costBudget removed entirely
    const contract = makeContract({ costBudget: undefined });
    const state = makeRunState({ retryCount: 1, wallClockElapsedSeconds: 60 });

    const program = new Command();
    program.exitOverride();
    const taskCmd = program.command("task");
    registerTaskBudgetCommand(taskCmd, program, {
      getServices: () => ({
        contractVersionStore: fakeContractVersionStore(contract),
        runStateStore: fakeRunStateStore(state),
        contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
      }),
    });

    await program.parseAsync(["task", "budget", "--task", "tsk-aaaaaa"], { from: "user" });
    const joined = lines.join("\n");
    expect(joined).toContain("(no costBudget set on contract");
    expect(joined).toContain("(no limit)");
    expect(joined).not.toContain("/0");
  });

  it("JSON mode reports hasBudget=false and omits max* fields when costBudget is undefined", async () => {
    let captured = "";
    process.stdout.write = ((data: string | Uint8Array) => {
      if (typeof data === "string") captured += data;
      return true;
    }) as typeof process.stdout.write;

    const contract = makeContract({ costBudget: undefined });
    const state = makeRunState({ retryCount: 1, wallClockElapsedSeconds: 60 });

    const program = new Command();
    program.exitOverride();
    const taskCmd = program.command("task");
    registerTaskBudgetCommand(taskCmd, program, {
      getServices: () => ({
        contractVersionStore: fakeContractVersionStore(contract),
        runStateStore: fakeRunStateStore(state),
        contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
      }),
    });

    await program.parseAsync(["task", "budget", "--task", "tsk-aaaaaa", "--json"], { from: "user" });

    const parsed = JSON.parse(captured);
    expect(parsed.hasBudget).toBe(false);
    expect(parsed.maxRetries).toBeUndefined();
    expect(parsed.maxWallClockSeconds).toBeUndefined();
  });

  it("text mode omits tokens row when maxTokens is undefined", async () => {
    const lines: string[] = [];
    console.log = (...args: unknown[]) => lines.push(args.map(String).join(" "));

    const contract = makeContract({ costBudget: { maxRetries: 5, maxWallClockSeconds: 3600 } });
    const state = makeRunState({ retryCount: 1, wallClockElapsedSeconds: 60 });

    const program = new Command();
    program.exitOverride();
    const taskCmd = program.command("task");
    registerTaskBudgetCommand(taskCmd, program, {
      getServices: () => ({
        contractVersionStore: fakeContractVersionStore(contract),
        runStateStore: fakeRunStateStore(state),
        contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
      }),
    });

    await program.parseAsync(["task", "budget", "--task", "tsk-aaaaaa"], { from: "user" });
    const joined = lines.join("\n");
    expect(joined).not.toContain("Tokens");
  });
});
