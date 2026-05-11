import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { writeFile, mkdtemp, rm } from "node:fs/promises";
import { Command } from "commander";
import { registerPlanCheckCommand } from "@/features/plan/commands/plan-check.command.js";
import type { ContractVersionStorePort } from "@/features/task/ports/contract-version-store.port.js";
import type { EvidenceRow, EvidenceStorePort } from "@/features/evidence/index.js";
import type { SpecStorePort } from "@/features/spec/ports/storage.js";
import type { Contract } from "@/features/task/index.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";

// ─── Console capture ──────────────────────────────────────────────────────────

const originalConsoleLog = console.log;

afterEach(() => {
  console.log = originalConsoleLog;
});

function captureConsole(): { logs: string[] } {
  const logs: string[] = [];
  console.log = (...args: unknown[]) => { logs.push(args.map(String).join(" ")); };
  return { logs };
}

// ─── Factories ─────────────────────────────────────────────────────────────────

function makeContract(filesExpected: string[] = ["src/foo.ts"]): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-000001",
    taskId: "tsk-aaaaaa",
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-01-01T00:00:00.000Z",
    intent: "Test task",
    scope: { filesExpected, filesForbidden: [] },
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

function fakeContractVersionStore(contract?: Contract): ContractVersionStorePort {
  return {
    write: async () => {},
    readCurrent: async () => contract,
    readVersion: async () => contract,
    history: async () => (contract !== undefined ? [contract] : []),
  };
}

function fakeSpecStore(): SpecStorePort {
  return {
    write: async () => {},
    read: async () => undefined,
    list: async () => [],
  };
}

function fakeEvidenceStore(): EvidenceStorePort & { appended: EvidenceRow[] } {
  const appended: EvidenceRow[] = [];
  return {
    appended,
    append: async (row) => { appended.push(row); },
    read: async () => undefined,
    list: async () => [],
  };
}

// ─── Temp dir helpers ─────────────────────────────────────────────────────────

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "plan-check-test-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function writePlanFile(name: string, content: unknown): Promise<string> {
  const path = join(tmpDir, name);
  await writeFile(path, typeof content === "string" ? content : JSON.stringify(content), "utf8");
  return path;
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("registerPlanCheckCommand", () => {
  it("reads the JSON plan file, runs checks, and exits 0 with findings printed", async () => {
    const contract = makeContract(["src/foo.ts"]);
    const evidenceStore = fakeEvidenceStore();

    const services = {
      contractVersionStore: fakeContractVersionStore(contract),
      evidenceStore,
      specStore: fakeSpecStore(),
      contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
    };

    const planPath = await writePlanFile("plan.json", {
      intendedFiles: ["src/foo.ts"],
      proofSet: [],
      riskClass: "low",
    });

    const program = new Command().exitOverride();
    const planCmd = program.command("plan");
    registerPlanCheckCommand(planCmd, program, { getServices: () => services });

    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "plan", "check", "--task", "tsk-aaaaaa", "--plan-file", planPath]);

    expect(logs.some((l) => l.includes("[ok]") || l.includes("Plan check"))).toBe(true);
  });

  it("--json flag produces parseable JSON output", async () => {
    const contract = makeContract(["src/foo.ts"]);
    const evidenceStore = fakeEvidenceStore();

    const services = {
      contractVersionStore: fakeContractVersionStore(contract),
      evidenceStore,
      specStore: fakeSpecStore(),
      contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
    };

    const planPath = await writePlanFile("plan.json", {
      intendedFiles: ["src/foo.ts"],
      proofSet: [],
      riskClass: "low",
    });

    const program = new Command().exitOverride();
    const planCmd = program.command("plan");
    registerPlanCheckCommand(planCmd, program, { getServices: () => services });

    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "plan", "check", "--task", "tsk-aaaaaa", "--plan-file", planPath, "--json"]);

    expect(logs).toHaveLength(1);
    const parsed = JSON.parse(logs[0]!);
    expect(parsed).toHaveProperty("findings");
    expect(parsed).toHaveProperty("errorCount");
    expect(parsed).toHaveProperty("warnCount");
  });

  it("records an evidence row of kind=plan-check with the findings payload", async () => {
    const contract = makeContract(["src/foo.ts"]);
    const evidenceStore = fakeEvidenceStore();

    const services = {
      contractVersionStore: fakeContractVersionStore(contract),
      evidenceStore,
      specStore: fakeSpecStore(),
      contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
    };

    const planPath = await writePlanFile("plan.json", {
      intendedFiles: ["src/foo.ts", "src/extra.ts"],
      proofSet: [],
      riskClass: "low",
    });

    const program = new Command().exitOverride();
    const planCmd = program.command("plan");
    registerPlanCheckCommand(planCmd, program, { getServices: () => services });

    captureConsole();

    await program.parseAsync(["node", "maestro", "plan", "check", "--task", "tsk-aaaaaa", "--plan-file", planPath]);

    expect(evidenceStore.appended).toHaveLength(1);
    const row = evidenceStore.appended[0]!;
    expect(row.kind).toBe("plan-check");
    expect(row.task_id).toBe("tsk-aaaaaa");
    expect(row.witness_level).toBe("agent-claimed-locally");

    const payload = row.payload as { planFileSha: string; findings: unknown[]; errorCount: number; warnCount: number };
    expect(typeof payload.planFileSha).toBe("string");
    expect(payload.planFileSha).toHaveLength(64); // SHA-256 hex
    expect(payload.errorCount).toBeGreaterThan(0);
    expect(Array.isArray(payload.findings)).toBe(true);
  });

  it("evidence row findings carry scope-widens finding when plan exceeds contract scope", async () => {
    const contract = makeContract(["src/foo.ts"]);
    const evidenceStore = fakeEvidenceStore();

    const services = {
      contractVersionStore: fakeContractVersionStore(contract),
      evidenceStore,
      specStore: fakeSpecStore(),
      contractStore: { get: async () => undefined, getByTaskId: async () => undefined, all: async () => [], readIndex: async () => [], create: async () => { throw new Error("Not implemented"); }, save: async () => { throw new Error("Not implemented"); }, delete: async () => false },
    };

    const planPath = await writePlanFile("plan.json", {
      intendedFiles: ["src/foo.ts", "src/secret.ts"],
      proofSet: [],
      riskClass: "low",
    });

    const program = new Command().exitOverride();
    const planCmd = program.command("plan");
    registerPlanCheckCommand(planCmd, program, { getServices: () => services });

    captureConsole();

    await program.parseAsync(["node", "maestro", "plan", "check", "--task", "tsk-aaaaaa", "--plan-file", planPath]);

    const payload = evidenceStore.appended[0]!.payload as { findings: Array<{ check: string }> };
    expect(payload.findings.some((f) => f.check === "scope-widens")).toBe(true);
  });
});
