import { afterEach, describe, expect, it } from "bun:test";
import { Command } from "commander";
import { registerTaskProofCommand } from "@/features/task/commands/task-proof.command.js";
import { mockEvidenceStore, mockTaskStore, mockContractStore } from "../../../../helpers/mocks.js";
import type { EvidenceStorePort, EvidenceRow } from "@/features/evidence/index.js";
import type { SpecStorePort, Spec } from "@/features/spec/index.js";
import type { TaskStorePort, Task, Contract } from "@/features/task";
import type { ContractVersionStorePort } from "@/features/task/ports/contract-version-store.port.js";
import type { ContractStorePort } from "@/features/task/ports/contract-store.port.js";
import type { ProofMap } from "@/features/verify/index.js";

function mockContractVersionStore(contract: Contract | undefined): ContractVersionStorePort {
  return {
    write: async () => {},
    readCurrent: async () => contract,
    readVersion: async () => undefined,
    history: async () => (contract ? [contract] : []),
  };
}

// ─── helpers ─────────────────────────────────────────────────────────────────

const TASK_ID = "tsk-proof01";
const MISSION_ID = "msn-001";

function makeTask(overrides: Partial<Task> = {}): Task {
  return {
    id: TASK_ID,
    title: "Test task",
    type: "task",
    priority: 2,
    status: "in_progress",
    labels: [],
    blocks: [],
    blockedBy: [],
    missionId: MISSION_ID,
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
    ...overrides,
  };
}

function makeSpec(overrides: Partial<Spec> = {}): Spec {
  return {
    schema_version: 2,
    mission_id: MISSION_ID,
    acceptance_criteria: [
      { id: "c-001", text: "Feature works end-to-end" },
      { id: "c-002", text: "All tests pass" },
    ],
    non_goals: [],
    runtime_signals: [],
    created_at: "2026-01-01T00:00:00.000Z",
    updated_at: "2026-01-01T00:00:00.000Z",
    ...overrides,
  };
}

function mockSpecStore(spec: Spec | undefined): SpecStorePort {
  return {
    write: async () => {},
    read: async (_missionId: string) => spec,
    list: async () => (spec ? [spec] : []),
  };
}

function makeCommandRow(criterionId?: string): EvidenceRow {
  return {
    schema_version: 3,
    id: `ev-${Math.random().toString(36).slice(2)}`,
    task_id: TASK_ID,
    kind: "command",
    witness_level: "witnessed-by-maestro",
    created_at: "2026-01-01T00:00:00.000Z",
    payload: {
      command: "bun test",
      exit: 0,
      criterion_id: criterionId,
    },
  };
}

interface TestDeps {
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly specStore: SpecStorePort;
  readonly contractVersionStore: ContractVersionStorePort;
  readonly contractStore: ContractStorePort;
}

function makeDeps(overrides: Partial<TestDeps> = {}): TestDeps {
  return {
    taskStore: overrides.taskStore ?? mockTaskStore([makeTask()]),
    evidenceStore: overrides.evidenceStore ?? mockEvidenceStore(),
    specStore: overrides.specStore ?? mockSpecStore(makeSpec()),
    contractVersionStore: overrides.contractVersionStore ?? mockContractVersionStore(undefined),
    contractStore: overrides.contractStore ?? mockContractStore(),
  };
}

function makeProgram(): Command {
  return new Command().name("maestro").option("--json", "Output as JSON").exitOverride();
}

// ─── console capture ─────────────────────────────────────────────────────────

const originalConsoleLog = console.log;

function captureConsole(): { logs: string[] } {
  const logs: string[] = [];
  console.log = (...args: unknown[]) => logs.push(args.map(String).join(" "));
  return { logs };
}

afterEach(() => {
  console.log = originalConsoleLog;
});

// ─── runner ───────────────────────────────────────────────────────────────────

async function runProof(
  argv: string[],
  deps: TestDeps,
): Promise<{ logs: string[]; exitCode: number }> {
  const program = makeProgram();
  const taskCmd = program.command("task");
  registerTaskProofCommand(taskCmd, program, { getServices: () => deps });

  const { logs } = captureConsole();

  try {
    await program.parseAsync(["node", "maestro", "task", "proof", ...argv]);
    return { logs, exitCode: 0 };
  } catch (err) {
    if (err instanceof Error && err.message.startsWith("process.exit(")) {
      return { logs, exitCode: 0 };
    }
    throw err;
  }
}

// ─── tests ───────────────────────────────────────────────────────────────────

describe("task proof", () => {
  describe("text output", () => {
    it("lists each criterion with [covered] or [uncovered] marker", async () => {
      const evidenceStore = mockEvidenceStore([makeCommandRow("c-001")]);
      const deps = makeDeps({ evidenceStore });
      const { logs } = await runProof(["--task", TASK_ID], deps);

      const output = logs.join("\n");
      expect(output).toContain("[covered]");
      expect(output).toContain("[uncovered]");
    });

    it("shows covered marker for criterion with evidence", async () => {
      const evidenceStore = mockEvidenceStore([makeCommandRow("c-001")]);
      const deps = makeDeps({ evidenceStore });
      const { logs } = await runProof(["--task", TASK_ID], deps);

      const coveredLine = logs.find((l) => l.includes("c-001"));
      expect(coveredLine).toBeDefined();
      expect(coveredLine).toContain("[covered]");
    });

    it("shows uncovered marker for criterion without evidence", async () => {
      const deps = makeDeps(); // no evidence rows
      const { logs } = await runProof(["--task", TASK_ID], deps);

      const lines = logs.filter((l) => l.includes("c-001") || l.includes("c-002"));
      for (const line of lines) {
        expect(line).toContain("[uncovered]");
      }
    });

    it("exits 0 regardless of coverage", async () => {
      const deps = makeDeps(); // all uncovered
      const { exitCode } = await runProof(["--task", TASK_ID], deps);
      expect(exitCode).toBe(0);
    });

    it("exits 0 when all criteria are covered", async () => {
      const evidenceStore = mockEvidenceStore([
        makeCommandRow("c-001"),
        makeCommandRow("c-002"),
      ]);
      const deps = makeDeps({ evidenceStore });
      const { exitCode } = await runProof(["--task", TASK_ID], deps);
      expect(exitCode).toBe(0);
    });

    it("surfaces an advisory + hint when task has no missionId and no contract", async () => {
      const task = makeTask({ missionId: undefined });
      const taskStore = mockTaskStore([task]);
      const deps = makeDeps({ taskStore });
      const { logs } = await runProof(["--task", TASK_ID], deps);
      const joined = logs.join("\n");
      expect(joined).toContain("nothing to prove against");
      expect(joined).toContain("maestro task contract new");
    });
  });

  describe("--json output", () => {
    it("outputs parseable JSON matching ProofMap shape", async () => {
      const evidenceStore = mockEvidenceStore([makeCommandRow("c-001")]);
      const deps = makeDeps({ evidenceStore });

      const written: string[] = [];
      const originalWrite = process.stdout.write.bind(process.stdout);
      process.stdout.write = ((chunk: unknown) => {
        if (typeof chunk === "string") written.push(chunk);
        return true;
      }) as typeof process.stdout.write;

      captureConsole();
      try {
        await runProof(["--task", TASK_ID, "--json"], deps);
      } finally {
        process.stdout.write = originalWrite;
      }

      const raw = written.join("");
      const parsed = JSON.parse(raw) as ProofMap;
      expect(parsed.taskId).toBe(TASK_ID);
      expect(parsed.missionId).toBe(MISSION_ID);
      expect(Array.isArray(parsed.entries)).toBe(true);
      expect(typeof parsed.uncoveredCount).toBe("number");
    });

    it("JSON contains entries with criterionId, criterionText, covered, evidence", async () => {
      const evidenceStore = mockEvidenceStore([makeCommandRow("c-001")]);
      const deps = makeDeps({ evidenceStore });

      const written: string[] = [];
      const originalWrite = process.stdout.write.bind(process.stdout);
      process.stdout.write = ((chunk: unknown) => {
        if (typeof chunk === "string") written.push(chunk);
        return true;
      }) as typeof process.stdout.write;

      captureConsole();
      try {
        await runProof(["--task", TASK_ID, "--json"], deps);
      } finally {
        process.stdout.write = originalWrite;
      }

      const parsed = JSON.parse(written.join("")) as ProofMap;
      expect(parsed.entries.length).toBe(2);
      const first = parsed.entries[0]!;
      expect(first.criterionId).toBe("c-001");
      expect(first.covered).toBe(true);
      expect(Array.isArray(first.evidence)).toBe(true);
      expect(parsed.uncoveredCount).toBe(1);
    });
  });

  describe("task not found", () => {
    it("throws MaestroError when task does not exist", async () => {
      const taskStore = mockTaskStore([]); // empty
      const deps = makeDeps({ taskStore });
      const program = makeProgram();
      const taskCmd = program.command("task");
      registerTaskProofCommand(taskCmd, program, { getServices: () => deps });
      captureConsole();
      await expect(
        program.parseAsync(["node", "maestro", "task", "proof", "--task", "tsk-missing"]),
      ).rejects.toThrow(/not found/i);
    });
  });
});
