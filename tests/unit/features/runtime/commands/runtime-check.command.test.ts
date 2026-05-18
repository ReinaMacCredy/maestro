import { describe, expect, it } from "bun:test";
import { Command } from "commander";
import { registerRuntimeCheckCommand } from "@/features/runtime/commands/runtime-check.command.js";
import type { RuntimeMonitorPort } from "@/features/runtime/ports/monitor.port.js";
import type { RuntimeSignalResult } from "@/features/runtime/domain/types.js";
import type { EvidenceStorePort, EvidenceRow, RecordEvidenceInput } from "@/features/evidence/index.js";
import type { RuntimeSignalPayload } from "@/features/evidence/index.js";
import type { TaskStorePort, Task } from "@/shared/domain/task";
import type { LegacySpecStorePort as SpecStorePort, Spec } from "@/shared/domain/legacy-spec/index.js";
import { mockEvidenceStore, mockTaskStore } from "../../../../helpers/mocks.js";

const STUB_TASK: Task = {
  id: "tsk-aaaaaa",
  title: "Stub task",
  type: "task",
  status: "pending",
  priority: 2,
  labels: [],
  blocks: [],
  blockedBy: [],
  missionId: "msn-bbbbbb",
  createdAt: "2026-05-05T00:00:00.000Z",
  updatedAt: "2026-05-05T00:00:00.000Z",
};

function makeSpec(runtime_signals: Spec["runtime_signals"] = []): Spec {
  return {
    schema_version: 2,
    mission_id: "msn-bbbbbb",
    acceptance_criteria: [],
    non_goals: [],
    runtime_signals,
    created_at: "2026-05-05T00:00:00.000Z",
    updated_at: "2026-05-05T00:00:00.000Z",
  };
}

function mockSpecStore(initial: Spec[] = []): SpecStorePort {
  const store = new Map(initial.map((s) => [s.mission_id, s]));
  return {
    write: async (spec) => { store.set(spec.mission_id, spec); },
    read: async (missionId) => store.get(missionId),
    list: async () => [...store.values()],
  };
}

function stubMonitor(result: RuntimeSignalResult | Error): RuntimeMonitorPort {
  return {
    query: async () => {
      if (result instanceof Error) throw result;
      return result;
    },
  };
}

function makeProgram(opts: {
  evidenceStore: EvidenceStorePort;
  taskStore: TaskStorePort;
  specStore: SpecStorePort;
  monitor?: RuntimeMonitorPort;
}): Command {
  const { evidenceStore, taskStore, specStore, monitor } = opts;

  const program = new Command().exitOverride();

  const runtimeCmd = program
    .command("runtime")
    .description("Runtime commands");

  registerRuntimeCheckCommand(runtimeCmd, program, {
    getServices: () => ({ legacyEvidenceStore: evidenceStore, legacyTaskStore: taskStore, trustSpecStore: specStore }),
    recordEvidence: async <K extends import("@/features/evidence/index.js").EvidenceKind>(
      s: EvidenceStorePort,
      input: RecordEvidenceInput<K>,
    ): Promise<EvidenceRow<K>> => {
      const row: EvidenceRow<K> = {
        schema_version: 3,
        id: `evd-test-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
        task_id: input.task_id,
        kind: input.kind,
        witness_level: input.witness_level,
        created_at: new Date().toISOString(),
        payload: input.payload,
      };
      await s.append(row);
      return row;
    },
    buildMonitor: (_baseUrl) => monitor ?? stubMonitor({ value: 0.005, threshold: 0.01, operator: "<", pass: true, sampled_at: "2026-05-05T00:00:00.000Z" }),
  });

  return program;
}

describe("runtime check command", () => {
  it("writes one runtime-signal Evidence row per prometheus signal", async () => {
    const evidenceStore = mockEvidenceStore();
    const taskStore = mockTaskStore([STUB_TASK]);
    const spec = makeSpec([
      {
        name: "error-rate",
        provider: "prometheus",
        query: "rate(errors_total[5m])",
        threshold: { operator: "<", value: 0.01 },
        severity: "critical",
      },
    ]);
    const specStore = mockSpecStore([spec]);
    const monitorResult: RuntimeSignalResult = {
      value: 0.005,
      threshold: 0.01,
      operator: "<",
      pass: true,
      sampled_at: "2026-05-05T00:00:00.000Z",
    };

    const program = makeProgram({ evidenceStore, taskStore, specStore, monitor: stubMonitor(monitorResult) });

    await program.parseAsync(["node", "maestro", "runtime", "check", "--task", "tsk-aaaaaa"]);

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa" });
    expect(rows).toHaveLength(1);

    const row = rows[0]!;
    expect(row.kind).toBe("runtime-signal");
    expect(row.witness_level).toBe("agent-claimed-locally");

    const payload = row.payload as RuntimeSignalPayload;
    expect(payload.signal_name).toBe("error-rate");
    expect(payload.provider).toBe("prometheus");
    expect(payload.value).toBe(0.005);
    expect(payload.pass).toBe(true);
    expect(payload.note).toBeUndefined();
  });

  it("skips unknown provider and records pass=false Evidence row with note", async () => {
    const evidenceStore = mockEvidenceStore();
    const taskStore = mockTaskStore([STUB_TASK]);
    const spec = makeSpec([
      {
        name: "dd-signal",
        provider: "datadog",
        query: "avg:system.cpu.user{*}",
        threshold: { operator: "<", value: 80 },
        severity: "warn",
      },
    ]);
    const specStore = mockSpecStore([spec]);

    let monitorCalled = false;
    const monitor: RuntimeMonitorPort = {
      query: async () => { monitorCalled = true; return { value: 0, threshold: 80, operator: "<", pass: false, sampled_at: "" }; },
    };

    const program = makeProgram({ evidenceStore, taskStore, specStore, monitor });

    await program.parseAsync(["node", "maestro", "runtime", "check", "--task", "tsk-aaaaaa"]);

    expect(monitorCalled).toBe(false);

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa" });
    expect(rows).toHaveLength(1);

    const payload = rows[0]!.payload as RuntimeSignalPayload;
    expect(payload.pass).toBe(false);
    expect(payload.note).toBe("unsupported provider");
  });

  it("records pass=false Evidence row with error note when monitor.query throws", async () => {
    const evidenceStore = mockEvidenceStore();
    const taskStore = mockTaskStore([STUB_TASK]);
    const spec = makeSpec([
      {
        name: "error-rate",
        provider: "prometheus",
        query: "rate(errors_total[5m])",
        threshold: { operator: "<", value: 0.01 },
        severity: "critical",
      },
    ]);
    const specStore = mockSpecStore([spec]);
    const monitor = stubMonitor(new Error("connection refused"));

    const program = makeProgram({ evidenceStore, taskStore, specStore, monitor });

    await program.parseAsync(["node", "maestro", "runtime", "check", "--task", "tsk-aaaaaa"]);

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa" });
    expect(rows).toHaveLength(1);

    const payload = rows[0]!.payload as RuntimeSignalPayload;
    expect(payload.pass).toBe(false);
    expect(payload.note).toBe("error: connection refused");
  });

  it("exits 0 when all signals pass", async () => {
    const evidenceStore = mockEvidenceStore();
    const taskStore = mockTaskStore([STUB_TASK]);
    const spec = makeSpec([
      {
        name: "latency",
        provider: "prometheus",
        query: "histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m]))",
        threshold: { operator: "<", value: 0.5 },
        severity: "critical",
      },
    ]);
    const specStore = mockSpecStore([spec]);
    const monitor = stubMonitor({ value: 0.1, threshold: 0.5, operator: "<", pass: true, sampled_at: "2026-05-05T00:00:00.000Z" });

    const program = makeProgram({ evidenceStore, taskStore, specStore, monitor });

    // exits 0 — no exception thrown
    await expect(
      program.parseAsync(["node", "maestro", "runtime", "check", "--task", "tsk-aaaaaa"]),
    ).resolves.toBeDefined();
  });

  it("exits 0 even when a signal fails", async () => {
    const evidenceStore = mockEvidenceStore();
    const taskStore = mockTaskStore([STUB_TASK]);
    const spec = makeSpec([
      {
        name: "error-rate",
        provider: "prometheus",
        query: "rate(errors_total[5m])",
        threshold: { operator: "<", value: 0.01 },
        severity: "critical",
      },
    ]);
    const specStore = mockSpecStore([spec]);
    const monitor = stubMonitor({ value: 0.1, threshold: 0.01, operator: "<", pass: false, sampled_at: "2026-05-05T00:00:00.000Z" });

    const program = makeProgram({ evidenceStore, taskStore, specStore, monitor });

    // verb always exits 0
    await expect(
      program.parseAsync(["node", "maestro", "runtime", "check", "--task", "tsk-aaaaaa"]),
    ).resolves.toBeDefined();
  });

  it("exits 0 even for unsupported provider (skip path)", async () => {
    const evidenceStore = mockEvidenceStore();
    const taskStore = mockTaskStore([STUB_TASK]);
    const spec = makeSpec([
      {
        name: "dd",
        provider: "datadog",
        query: "avg:foo{*}",
        threshold: { operator: "<", value: 1 },
        severity: "info",
      },
    ]);
    const specStore = mockSpecStore([spec]);

    const program = makeProgram({ evidenceStore, taskStore, specStore });

    await expect(
      program.parseAsync(["node", "maestro", "runtime", "check", "--task", "tsk-aaaaaa"]),
    ).resolves.toBeDefined();
  });

  it("throws MaestroError when task not found", async () => {
    const evidenceStore = mockEvidenceStore();
    const taskStore = mockTaskStore([]);
    const specStore = mockSpecStore([]);

    const program = makeProgram({ evidenceStore, taskStore, specStore });

    await expect(
      program.parseAsync(["node", "maestro", "runtime", "check", "--task", "tsk-ffffff"]),
    ).rejects.toThrow("Task not found");
  });
});
