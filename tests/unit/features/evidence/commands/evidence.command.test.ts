import { afterEach, describe, expect, it } from "bun:test";
import { Command } from "commander";
import {
  registerEvidenceCommand,
  recordEvidence as realRecordEvidence,
  type EvidenceRow,
  type EvidenceStorePort,
  type RecordEvidenceInput,
  type AIReviewPayload,
  type DeployReadinessPayload,
  type RollbackExercisedPayload,
  type RuntimeSignalPayload,
  type ThreatModelPayload,
} from "@/features/evidence";
import { join } from "node:path";
import { mockEvidenceStore } from "../../../../helpers/mocks.js";
import type { LegacyTask as Task, LegacyTaskStorePort as TaskStorePort } from "@/shared/domain/legacy-task";
import type { LegacySpecStorePort as SpecStorePort, Spec } from "@/shared/domain/legacy-spec/index.js";

const originalConsoleLog = console.log;
const originalConsoleError = console.error;

function captureConsole(): {
  readonly logs: string[];
  readonly errors: string[];
} {
  const logs: string[] = [];
  const errors: string[] = [];
  console.log = (...args: unknown[]) => {
    logs.push(args.map((arg) => String(arg)).join(" "));
  };
  console.error = (...args: unknown[]) => {
    errors.push(args.map((arg) => String(arg)).join(" "));
  };
  return { logs, errors };
}

afterEach(() => {
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
});

function makeTask(id: string): Task {
  return {
    id,
    title: "test",
    type: "task",
    priority: 2,
    status: "pending",
    labels: [],
    blocks: [],
    blockedBy: [],
    createdAt: "2026-05-03T00:00:00.000Z",
    updatedAt: "2026-05-03T00:00:00.000Z",
  };
}

function fakeTaskStore(tasks: readonly Task[]): Pick<TaskStorePort, "get"> {
  const map = new Map(tasks.map((t) => [t.id, t] as const));
  return {
    get: async (id) => map.get(id),
  };
}

function mockSpecStore(initial: Spec[] = []): SpecStorePort {
  const store = new Map(initial.map((s) => [s.mission_id, s]));
  return {
    write: async (spec) => { store.set(spec.mission_id, spec); },
    read: async (missionId) => store.get(missionId),
    list: async () => [...store.values()].sort((a, b) => a.mission_id.localeCompare(b.mission_id)),
  };
}

interface DepsOverrides {
  readonly tasks?: readonly Task[];
  readonly evidenceStore?: EvidenceStorePort;
  readonly specStore?: SpecStorePort;
  readonly recordEvidence?: typeof realRecordEvidence;
}

function evidenceDeps(overrides: DepsOverrides = {}) {
  const tasks = overrides.tasks ?? [makeTask("tsk-aaaaaa")];
  const evidenceStore = overrides.evidenceStore ?? mockEvidenceStore();
  const specStore = overrides.specStore ?? mockSpecStore();
  return {
    deps: {
      getServices: () => ({
        evidenceStore,
        taskStore: fakeTaskStore(tasks) as TaskStorePort,
        specStore,
        contractVersionStore: { write: async () => {}, readCurrent: async () => undefined, readVersion: async () => undefined, history: async () => [] },
        contractStore: {
          get: async () => undefined,
          getByTaskId: async () => undefined,
          all: async () => [],
          readIndex: async () => [],
          create: async () => { throw new Error("Not implemented"); },
          save: async () => { throw new Error("Not implemented"); },
          delete: async () => false,
        },
        v2: {
          taskStore: {
            create: async () => { throw new Error("Not implemented"); },
            get: async () => undefined,
            update: async () => { throw new Error("Not implemented"); },
            list: async () => [],
            listByState: async () => [],
            listByMissionId: async () => [],
          },
        } as never,
      }),
      recordEvidence: overrides.recordEvidence ?? realRecordEvidence,
    },
    evidenceStore,
  };
}

function makeProgram(): Command {
  return new Command().name("maestro").option("--json", "Output as JSON");
}

describe("registerEvidenceCommand", () => {
  it("records command-kind evidence with witness level agent-claimed-locally", async () => {
    captureConsole();
    let received: RecordEvidenceInput | undefined;
    const { deps, evidenceStore } = evidenceDeps({
      recordEvidence: async (store, input) => {
        received = input;
        return realRecordEvidence(store, input);
      },
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync([
      "node",
      "maestro",
      "evidence",
      "record",
      "--task",
      "tsk-aaaaaa",
      "--command",
      "bun test",
      "--exit",
      "0",
    ]);

    expect(received).toBeDefined();
    expect(received!.task_id).toBe("tsk-aaaaaa");
    expect(received!.kind).toBe("command");
    expect(received!.session_id).toBeUndefined();
    expect(received!.witness_level).toBe("agent-claimed-locally");
    expect(received!.payload).toEqual({ command: "bun test", exit: 0 });

    const list = await evidenceStore.list({ task_id: "tsk-aaaaaa" });
    expect(list.length).toBe(1);
  });

  it("records manual-note kind with witness level agent-claimed-and-not-reproducible", async () => {
    captureConsole();
    let received: RecordEvidenceInput | undefined;
    const { deps } = evidenceDeps({
      recordEvidence: async (store, input) => {
        received = input;
        return realRecordEvidence(store, input);
      },
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync([
      "node",
      "maestro",
      "evidence",
      "record",
      "--task",
      "tsk-aaaaaa",
      "--kind",
      "manual-note",
      "--note",
      "verified",
    ]);

    expect(received!.kind).toBe("manual-note");
    expect(received!.witness_level).toBe("agent-claimed-and-not-reproducible");
    expect(received!.payload).toEqual({ note: "verified" });
  });

  it("forwards optional payload fields (log, duration, criterion)", async () => {
    captureConsole();
    let received: RecordEvidenceInput | undefined;
    const { deps } = evidenceDeps({
      recordEvidence: async (store, input) => {
        received = input;
        return realRecordEvidence(store, input);
      },
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync([
      "node",
      "maestro",
      "evidence",
      "record",
      "--task",
      "tsk-aaaaaa",
      "--command",
      "bun run build",
      "--exit",
      "0",
      "--log",
      "./out.log",
      "--duration",
      "1234",
      "--criterion",
      "ui-01",
    ]);

    expect(received!.payload).toEqual({
      command: "bun run build",
      exit: 0,
      log_path: "./out.log",
      duration_ms: 1234,
      criterion_id: "ui-01",
    });
  });

  it("--session attaches a session id to the evidence row", async () => {
    captureConsole();
    let received: RecordEvidenceInput | undefined;
    const { deps } = evidenceDeps({
      recordEvidence: async (store, input) => {
        received = input;
        return realRecordEvidence(store, input);
      },
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync([
      "node",
      "maestro",
      "evidence",
      "record",
      "--task",
      "tsk-aaaaaa",
      "--command",
      "bun test",
      "--exit",
      "0",
      "--session",
      "sess-override",
    ]);

    expect(received!.session_id).toBe("sess-override");
  });

  it("omits session_id when --session is not provided", async () => {
    captureConsole();
    let received: RecordEvidenceInput | undefined;
    const { deps } = evidenceDeps({
      recordEvidence: async (store, input) => {
        received = input;
        return realRecordEvidence(store, input);
      },
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync([
      "node",
      "maestro",
      "evidence",
      "record",
      "--task",
      "tsk-aaaaaa",
      "--command",
      "bun test",
      "--exit",
      "0",
    ]);

    expect(received!.session_id).toBeUndefined();
  });

  it("missing --task fails with a Commander error", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram().exitOverride();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync(["node", "maestro", "evidence", "record"]),
    ).rejects.toMatchObject({
      code: "commander.missingMandatoryOptionValue",
    });
  });

  it("rejects unknown task ids", async () => {
    captureConsole();
    const { deps } = evidenceDeps({ tasks: [] });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node",
        "maestro",
        "evidence",
        "record",
        "--task",
        "tsk-deadbe",
        "--command",
        "bun test",
        "--exit",
        "0",
      ]),
    ).rejects.toMatchObject({
      message: "Task not found: tsk-deadbe",
    });
  });

  it("rejects invalid --kind", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node",
        "maestro",
        "evidence",
        "record",
        "--task",
        "tsk-aaaaaa",
        "--kind",
        "screenshot",
        "--note",
        "x",
      ]),
    ).rejects.toMatchObject({
      message: "Invalid --kind: screenshot",
    });
  });

  it("--kind command without --command rejects", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node",
        "maestro",
        "evidence",
        "record",
        "--task",
        "tsk-aaaaaa",
        "--exit",
        "0",
      ]),
    ).rejects.toMatchObject({
      message: "--kind command requires --command and --exit",
    });
  });

  it("--kind command without --exit rejects", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node",
        "maestro",
        "evidence",
        "record",
        "--task",
        "tsk-aaaaaa",
        "--command",
        "bun test",
      ]),
    ).rejects.toMatchObject({
      message: "--kind command requires --command and --exit",
    });
  });

  it("--kind manual-note without --note rejects", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node",
        "maestro",
        "evidence",
        "record",
        "--task",
        "tsk-aaaaaa",
        "--kind",
        "manual-note",
      ]),
    ).rejects.toMatchObject({
      message: "--kind manual-note requires --note",
    });
  });

  it("--exit non-integer rejects via Commander parser error", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node",
        "maestro",
        "evidence",
        "record",
        "--task",
        "tsk-aaaaaa",
        "--command",
        "bun test",
        "--exit",
        "abc",
      ]),
    ).rejects.toMatchObject({
      message: "Invalid integer: abc",
    });
  });

  it("--json prints a parseable row", async () => {
    const captured = captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync([
      "node",
      "maestro",
      "evidence",
      "record",
      "--task",
      "tsk-aaaaaa",
      "--command",
      "bun test",
      "--exit",
      "0",
      "--json",
    ]);

    expect(captured.logs.length).toBe(1);
    const parsed = JSON.parse(captured.logs[0]!) as EvidenceRow;
    expect(parsed.task_id).toBe("tsk-aaaaaa");
    expect(parsed.kind).toBe("command");
    expect(parsed.witness_level).toBe("agent-claimed-locally");
    expect(parsed.payload).toEqual({ command: "bun test", exit: 0 });
    expect(typeof parsed.id).toBe("string");
  });

  it("default text output prints success line and detail", async () => {
    const captured = captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync([
      "node",
      "maestro",
      "evidence",
      "record",
      "--task",
      "tsk-aaaaaa",
      "--command",
      "bun test",
      "--exit",
      "0",
    ]);

    expect(captured.logs[0]).toMatch(/^\[ok\] Evidence recorded: evd-/);
    expect(captured.logs).toContain("  Task: tsk-aaaaaa");
    expect(captured.logs).toContain("  Kind: command");
    expect(captured.logs).toContain("  Command: bun test");
    expect(captured.logs).toContain("  Exit: 0");
    expect(captured.logs).toContain("  Witness: agent-claimed-locally");
  });
});

function makeEvidenceRow(
  overrides: Partial<EvidenceRow> & { id: string },
): EvidenceRow {
  return {
    schema_version: 1,
    task_id: "tsk-aaaaaa",
    kind: "command",
    witness_level: "agent-claimed-locally",
    created_at: "2026-05-03T00:00:00.000Z",
    payload: { command: "bun test", exit: 0 },
    ...overrides,
  } as EvidenceRow;
}

describe("evidence list", () => {
  it("returns rows in chronological order when --task <id> is provided", async () => {
    const rows = [
      makeEvidenceRow({ id: "evd-0000000000003-aaaaaa", created_at: "2026-05-03T00:00:03.000Z", task_id: "tsk-aaaaaa" }),
      makeEvidenceRow({ id: "evd-0000000000001-aaaaaa", created_at: "2026-05-03T00:00:01.000Z", task_id: "tsk-aaaaaa" }),
      makeEvidenceRow({ id: "evd-0000000000002-aaaaaa", created_at: "2026-05-03T00:00:02.000Z", task_id: "tsk-aaaaaa" }),
    ];
    const captured = captureConsole();
    const store = mockEvidenceStore(rows);
    const { deps } = evidenceDeps({ evidenceStore: store });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync(["node", "maestro", "evidence", "list", "--task", "tsk-aaaaaa"]);

    expect(captured.logs.length).toBe(3);
    // First line should have earliest created_at
    expect(captured.logs[0]).toContain("evd-0000000000001-aaaaaa");
    expect(captured.logs[1]).toContain("evd-0000000000002-aaaaaa");
    expect(captured.logs[2]).toContain("evd-0000000000003-aaaaaa");
  });

  it("filters by --kind manual-note", async () => {
    const rows = [
      makeEvidenceRow({ id: "evd-0000000000001-aaaaaa", kind: "command" }),
      makeEvidenceRow({
        id: "evd-0000000000002-aaaaaa",
        kind: "manual-note",
        payload: { note: "verified" },
        witness_level: "agent-claimed-and-not-reproducible",
      }),
    ];
    const captured = captureConsole();
    const store = mockEvidenceStore(rows);
    const { deps } = evidenceDeps({ evidenceStore: store });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync(["node", "maestro", "evidence", "list", "--kind", "manual-note"]);

    expect(captured.logs.length).toBe(1);
    expect(captured.logs[0]).toContain("evd-0000000000002-aaaaaa");
    expect(captured.logs[0]).toContain("manual-note");
  });

  it("returns all rows when no filters are provided", async () => {
    const rows = [
      makeEvidenceRow({ id: "evd-0000000000001-aaaaaa", task_id: "tsk-aaaaaa" }),
      makeEvidenceRow({ id: "evd-0000000000002-aaaaaa", task_id: "tsk-bbbbbb" }),
    ];
    const captured = captureConsole();
    const store = mockEvidenceStore(rows);
    const { deps } = evidenceDeps({ evidenceStore: store });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync(["node", "maestro", "evidence", "list"]);

    expect(captured.logs.length).toBe(2);
  });

  it("--json prints a parseable { items, v2_items? } payload", async () => {
    const rows = [
      makeEvidenceRow({ id: "evd-0000000000001-aaaaaa" }),
    ];
    const captured = captureConsole();
    const store = mockEvidenceStore(rows);
    const { deps } = evidenceDeps({ evidenceStore: store });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync(["node", "maestro", "evidence", "list", "--json"]);

    expect(captured.logs.length).toBe(1);
    const parsed = JSON.parse(captured.logs[0]!) as {
      items: EvidenceRow[];
      v2_items?: unknown[];
    };
    expect(Array.isArray(parsed.items)).toBe(true);
    expect(parsed.items.length).toBe(1);
    expect(parsed.items[0]!.id).toBe("evd-0000000000001-aaaaaa");
    expect(parsed.v2_items).toBeUndefined();
  });

  it("prints 'No evidence found.' when empty", async () => {
    const captured = captureConsole();
    const { deps } = evidenceDeps({ evidenceStore: mockEvidenceStore([]) });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync(["node", "maestro", "evidence", "list"]);

    expect(captured.logs).toContain("No evidence found.");
  });
});

describe("evidence show", () => {
  it("shows a row in text mode", async () => {
    const row = makeEvidenceRow({ id: "evd-0000000000001-aaaaaa" });
    const captured = captureConsole();
    const store = mockEvidenceStore([row]);
    const { deps } = evidenceDeps({ evidenceStore: store });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync(["node", "maestro", "evidence", "show", "evd-0000000000001-aaaaaa"]);

    expect(captured.logs[0]).toMatch(/^\[ok\] Evidence: evd-0000000000001-aaaaaa$/);
    expect(captured.logs).toContain("  Task: tsk-aaaaaa");
    expect(captured.logs).toContain("  Kind: command");
  });

  it("--json prints a parseable row", async () => {
    const row = makeEvidenceRow({ id: "evd-0000000000001-aaaaaa" });
    const captured = captureConsole();
    const store = mockEvidenceStore([row]);
    const { deps } = evidenceDeps({ evidenceStore: store });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync(["node", "maestro", "evidence", "show", "evd-0000000000001-aaaaaa", "--json"]);

    expect(captured.logs.length).toBe(1);
    const parsed = JSON.parse(captured.logs[0]!) as EvidenceRow;
    expect(parsed.id).toBe("evd-0000000000001-aaaaaa");
    expect(parsed.task_id).toBe("tsk-aaaaaa");
  });

  it("throws MaestroError for missing id", async () => {
    captureConsole();
    const { deps } = evidenceDeps({ evidenceStore: mockEvidenceStore([]) });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync(["node", "maestro", "evidence", "show", "evd-0000000000000-deadbe"]),
    ).rejects.toMatchObject({
      message: "Evidence not found: evd-0000000000000-deadbe",
    });
  });

  it("throws MaestroError for invalid id pattern", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync(["node", "maestro", "evidence", "show", "not-an-id"]),
    ).rejects.toMatchObject({
      message: "Invalid evidence id: not-an-id",
    });
  });

  // --- L2.0: --criterion enforcement when mission has a Spec ---

  it("requires --criterion when task belongs to a mission that has a Spec with criteria", async () => {
    captureConsole();
    const missionId = "2026-05-04-001";
    const criteria = [
      { id: "crt-0000000000001-aabbccdd", text: "Alpha" },
      { id: "crt-0000000000002-bbccddee", text: "Beta" },
      { id: "crt-0000000000003-ccddeeff", text: "Gamma" },
    ];
    const spec: Spec = {
      schema_version: 2,
      mission_id: missionId,
      acceptance_criteria: criteria,
      non_goals: [],
      runtime_signals: [],
      created_at: "2026-05-04T00:00:00.000Z",
      updated_at: "2026-05-04T00:00:00.000Z",
    };
    const task: Task = {
      ...makeTask("tsk-bbbbbb"),
      missionId,
    };
    const { deps } = evidenceDeps({
      tasks: [task],
      specStore: mockSpecStore([spec]),
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node", "maestro", "evidence", "record",
        "--task", "tsk-bbbbbb",
        "--command", "bun test",
        "--exit", "0",
      ]),
    ).rejects.toMatchObject({
      message: "--criterion required when task's mission has a Spec",
    });
  });

  it("error message includes all available criterion ids", async () => {
    captureConsole();
    const missionId = "2026-05-04-002";
    const id1 = "crt-0000000000001-aabbccdd";
    const id2 = "crt-0000000000002-bbccddee";
    const id3 = "crt-0000000000003-ccddeeff";
    const spec: Spec = {
      schema_version: 2,
      mission_id: missionId,
      acceptance_criteria: [
        { id: id1, text: "Alpha" },
        { id: id2, text: "Beta" },
        { id: id3, text: "Gamma" },
      ],
      non_goals: [],
      runtime_signals: [],
      created_at: "2026-05-04T00:00:00.000Z",
      updated_at: "2026-05-04T00:00:00.000Z",
    };
    const task: Task = { ...makeTask("tsk-cccccc"), missionId };
    const { deps } = evidenceDeps({
      tasks: [task],
      specStore: mockSpecStore([spec]),
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    let thrownError: unknown;
    try {
      await program.parseAsync([
        "node", "maestro", "evidence", "record",
        "--task", "tsk-cccccc",
        "--command", "bun test",
        "--exit", "0",
      ]);
    } catch (err) {
      thrownError = err;
    }

    expect(thrownError).toBeDefined();
    const hints = (thrownError as { hints?: string[] }).hints ?? [];
    expect(hints.length).toBeGreaterThan(0);
    const allHints = hints.join("\n");
    expect(allHints).toContain(id1);
    expect(allHints).toContain(id2);
    expect(allHints).toContain(id3);
  });

  it("succeeds when --criterion matches a valid criterion id", async () => {
    captureConsole();
    const missionId = "2026-05-04-003";
    const criterionId = "crt-0000000000001-aabbccdd";
    const spec: Spec = {
      schema_version: 2,
      mission_id: missionId,
      acceptance_criteria: [{ id: criterionId, text: "Alpha" }],
      non_goals: [],
      runtime_signals: [],
      created_at: "2026-05-04T00:00:00.000Z",
      updated_at: "2026-05-04T00:00:00.000Z",
    };
    const task: Task = { ...makeTask("tsk-dddddd"), missionId };
    const { deps, evidenceStore } = evidenceDeps({
      tasks: [task],
      specStore: mockSpecStore([spec]),
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await program.parseAsync([
      "node", "maestro", "evidence", "record",
      "--task", "tsk-dddddd",
      "--command", "bun test",
      "--exit", "0",
      "--criterion", criterionId,
    ]);

    const rows = await evidenceStore.list({ task_id: "tsk-dddddd" });
    expect(rows.length).toBe(1);
    const payload = rows[0]!.payload as { criterion_id?: string };
    expect(payload.criterion_id).toBe(criterionId);
  });

  it("accepts evidence record without --criterion when task has no missionId", async () => {
    captureConsole();
    // Task without missionId -- current behavior preserved
    const task = makeTask("tsk-eeeeee");
    const { deps, evidenceStore } = evidenceDeps({ tasks: [task] });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await program.parseAsync([
      "node", "maestro", "evidence", "record",
      "--task", "tsk-eeeeee",
      "--command", "bun test",
      "--exit", "0",
    ]);

    const rows = await evidenceStore.list({ task_id: "tsk-eeeeee" });
    expect(rows.length).toBe(1);
  });

  it("accepts evidence record without --criterion when task mission has no Spec", async () => {
    captureConsole();
    // Task with a missionId but no spec for that mission
    const task: Task = { ...makeTask("tsk-ffffff"), missionId: "2026-05-04-999" };
    const { deps, evidenceStore } = evidenceDeps({
      tasks: [task],
      specStore: mockSpecStore([]), // no spec for this mission
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await program.parseAsync([
      "node", "maestro", "evidence", "record",
      "--task", "tsk-ffffff",
      "--command", "bun test",
      "--exit", "0",
    ]);

    const rows = await evidenceStore.list({ task_id: "tsk-ffffff" });
    expect(rows.length).toBe(1);
  });
});

// --- L4.3: ai-review Evidence kind ---

describe("evidence record --kind ai-review", () => {
  it("records an ai-review row with inline JSON findings", async () => {
    captureConsole();
    let received: RecordEvidenceInput | undefined;
    const { deps, evidenceStore } = evidenceDeps({
      recordEvidence: async (store, input) => {
        received = input;
        return realRecordEvidence(store, input);
      },
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync([
      "node", "maestro", "evidence", "record",
      "--task", "tsk-aaaaaa",
      "--kind", "ai-review",
      "--reviewer", "security",
      "--findings", '[{"severity":"error","message":"SQL injection risk"}]',
      "--confidence", "0.9",
    ]);

    expect(received).toBeDefined();
    expect(received!.kind).toBe("ai-review");
    expect(received!.witness_level).toBe("agent-claimed-locally");
    const payload = received!.payload as AIReviewPayload;
    expect(payload.reviewer).toBe("security");
    expect(payload.confidence).toBe(0.9);
    expect(payload.findings).toHaveLength(1);
    expect(payload.findings[0]!.severity).toBe("error");
    expect(payload.findings[0]!.message).toBe("SQL injection risk");

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa" });
    expect(rows.length).toBe(1);
  });

  it("errors when --reviewer is missing for --kind ai-review", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node", "maestro", "evidence", "record",
        "--task", "tsk-aaaaaa",
        "--kind", "ai-review",
        "--findings", '[{"severity":"info","message":"ok"}]',
      ]),
    ).rejects.toMatchObject({
      message: expect.stringContaining("--kind ai-review requires --reviewer"),
    });
  });

  it("errors when --reviewer is an invalid value", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node", "maestro", "evidence", "record",
        "--task", "tsk-aaaaaa",
        "--kind", "ai-review",
        "--reviewer", "unknown-reviewer",
        "--findings", '[{"severity":"info","message":"ok"}]',
      ]),
    ).rejects.toMatchObject({
      message: expect.stringContaining("--kind ai-review requires --reviewer"),
    });
  });

  it("errors when --findings is missing for --kind ai-review", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node", "maestro", "evidence", "record",
        "--task", "tsk-aaaaaa",
        "--kind", "ai-review",
        "--reviewer", "bug",
      ]),
    ).rejects.toMatchObject({
      message: "--kind ai-review requires --findings",
    });
  });

  it("errors when --confidence is outside [0,1]", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node", "maestro", "evidence", "record",
        "--task", "tsk-aaaaaa",
        "--kind", "ai-review",
        "--reviewer", "bug",
        "--findings", '[{"severity":"info","message":"ok"}]',
        "--confidence", "1.5",
      ]),
    ).rejects.toMatchObject({
      message: expect.stringContaining("--confidence must be between 0 and 1"),
    });
  });

  it("errors when a finding has an invalid severity", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node", "maestro", "evidence", "record",
        "--task", "tsk-aaaaaa",
        "--kind", "ai-review",
        "--reviewer", "architecture",
        "--findings", '[{"severity":"critical","message":"bad"}]',
      ]),
    ).rejects.toMatchObject({
      message: expect.stringContaining("severity must be one of: info, warn, error"),
    });
  });

  it("errors when a finding has an empty message", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node", "maestro", "evidence", "record",
        "--task", "tsk-aaaaaa",
        "--kind", "ai-review",
        "--reviewer", "bug",
        "--findings", '[{"severity":"warn","message":""}]',
      ]),
    ).rejects.toMatchObject({
      message: expect.stringContaining("message must be a non-empty string"),
    });
  });

  it("uses default confidence of 0.5 when not specified", async () => {
    captureConsole();
    let received: RecordEvidenceInput | undefined;
    const { deps } = evidenceDeps({
      recordEvidence: async (store, input) => {
        received = input;
        return realRecordEvidence(store, input);
      },
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync([
      "node", "maestro", "evidence", "record",
      "--task", "tsk-aaaaaa",
      "--kind", "ai-review",
      "--reviewer", "architecture",
      "--findings", '[{"severity":"info","message":"Looks good"}]',
    ]);

    const payload = received!.payload as AIReviewPayload;
    expect(payload.confidence).toBe(0.5);
  });

  it("accepts all three reviewer kinds: bug, security, architecture", async () => {
    for (const reviewer of ["bug", "security", "architecture"] as const) {
      captureConsole();
      let received: RecordEvidenceInput | undefined;
      const { deps } = evidenceDeps({
        recordEvidence: async (store, input) => {
          received = input;
          return realRecordEvidence(store, input);
        },
      });

      const program = makeProgram();
      registerEvidenceCommand(program, deps);
      await program.parseAsync([
        "node", "maestro", "evidence", "record",
        "--task", "tsk-aaaaaa",
        "--kind", "ai-review",
        "--reviewer", reviewer,
        "--findings", '[{"severity":"info","message":"ok"}]',
      ]);

      const payload = received!.payload as AIReviewPayload;
      expect(payload.reviewer).toBe(reviewer);
    }
  });
});

// --- L4.3a: threat-model Evidence kind ---

const FIXTURES_DIR = join(import.meta.dir, "../../../../fixtures/threat-models");

describe("evidence record --kind threat-model", () => {
  it("records a threat-model row with all payload fields from a JSON file", async () => {
    captureConsole();
    let received: RecordEvidenceInput | undefined;
    const { deps, evidenceStore } = evidenceDeps({
      recordEvidence: async (store, input) => {
        received = input;
        return realRecordEvidence(store, input);
      },
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync([
      "node", "maestro", "evidence", "record",
      "--task", "tsk-aaaaaa",
      "--kind", "threat-model",
      "--threat-model-file", join(FIXTURES_DIR, "minimal.json"),
    ]);

    expect(received).toBeDefined();
    expect(received!.kind).toBe("threat-model");
    expect(received!.witness_level).toBe("agent-claimed-locally");
    const payload = received!.payload as ThreatModelPayload;
    expect(payload.assets).toEqual(["session tokens", "password hashes"]);
    expect(payload.threatCategories).toEqual(["spoofing", "tampering", "info-disclosure"]);
    expect(payload.mitigations).toHaveLength(2);
    expect(payload.mitigations[0]).toEqual({ threat: "session-fixation", mitigation: "rotate token on login" });
    expect(payload.residualRisk).toBe("low");
    expect(payload.source_file).toBe(join(FIXTURES_DIR, "minimal.json"));

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa" });
    expect(rows.length).toBe(1);
    expect(rows[0]!.kind).toBe("threat-model");
  });

  it("accepts a YAML threat-model file", async () => {
    captureConsole();
    let received: RecordEvidenceInput | undefined;
    const { deps } = evidenceDeps({
      recordEvidence: async (store, input) => {
        received = input;
        return realRecordEvidence(store, input);
      },
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync([
      "node", "maestro", "evidence", "record",
      "--task", "tsk-aaaaaa",
      "--kind", "threat-model",
      "--threat-model-file", join(FIXTURES_DIR, "minimal.yaml"),
    ]);

    expect(received!.kind).toBe("threat-model");
    const payload = received!.payload as ThreatModelPayload;
    expect(payload.assets).toEqual(["session tokens", "password hashes"]);
    expect(payload.residualRisk).toBe("low");
    expect(payload.mitigations).toHaveLength(2);
  });

  it("errors when --threat-model-file is missing for --kind threat-model", async () => {
    captureConsole();
    const { deps } = evidenceDeps();

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node", "maestro", "evidence", "record",
        "--task", "tsk-aaaaaa",
        "--kind", "threat-model",
      ]),
    ).rejects.toMatchObject({
      message: "--kind threat-model requires --threat-model-file",
    });
  });

  it("errors when threat-model file has wrong type for assets field", async () => {
    captureConsole();
    const { deps } = evidenceDeps();
    const { writeFileSync, mkdtempSync } = await import("node:fs");
    const { tmpdir } = await import("node:os");
    const dir = mkdtempSync(join(tmpdir(), "tm-test-"));
    const malformed = join(dir, "bad.json");
    writeFileSync(malformed, JSON.stringify({
      assets: "not-an-array",
      threatCategories: [],
      mitigations: [],
      residualRisk: "low",
    }));

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node", "maestro", "evidence", "record",
        "--task", "tsk-aaaaaa",
        "--kind", "threat-model",
        "--threat-model-file", malformed,
      ]),
    ).rejects.toMatchObject({
      message: expect.stringContaining('"assets"'),
    });
  });

  it("errors when threat-model file is missing required field mitigations", async () => {
    captureConsole();
    const { deps } = evidenceDeps();
    const { writeFileSync, mkdtempSync } = await import("node:fs");
    const { tmpdir } = await import("node:os");
    const dir = mkdtempSync(join(tmpdir(), "tm-test-"));
    const malformed = join(dir, "no-mitigations.json");
    writeFileSync(malformed, JSON.stringify({
      assets: ["session tokens"],
      threatCategories: ["spoofing"],
      residualRisk: "medium",
    }));

    const program = makeProgram();
    registerEvidenceCommand(program, deps);

    await expect(
      program.parseAsync([
        "node", "maestro", "evidence", "record",
        "--task", "tsk-aaaaaa",
        "--kind", "threat-model",
        "--threat-model-file", malformed,
      ]),
    ).rejects.toMatchObject({
      message: expect.stringContaining('"mitigations"'),
    });
  });

  it("--witness witnessed-by-ci sets the witness level on the row", async () => {
    captureConsole();
    let received: RecordEvidenceInput | undefined;
    const { deps } = evidenceDeps({
      recordEvidence: async (store, input) => {
        received = input;
        return realRecordEvidence(store, input);
      },
    });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync([
      "node", "maestro", "evidence", "record",
      "--task", "tsk-aaaaaa",
      "--kind", "threat-model",
      "--threat-model-file", join(FIXTURES_DIR, "minimal.json"),
      "--witness", "witnessed-by-ci",
    ]);

    expect(received!.witness_level).toBe("witnessed-by-ci");
  });
});

describe("formatEvidenceRow — L7 kinds", () => {
  it("renders deploy-readiness with gate and check summary", async () => {
    const payload: DeployReadinessPayload = {
      task_id: "tsk-aaaaaa",
      checks: {
        feature_flag: { ok: true, value: "my-flag" },
        canary_plan:  { ok: false },
        rollback:     { ok: true, witness_evidence_id: "evd-0000000000002-b01bac" },
        owner:        { ok: false },
      },
      gate: "fail",
    };
    const row = makeEvidenceRow({
      id: "evd-0000000000010-de9ead",
      kind: "deploy-readiness",
      payload,
    });
    const captured = captureConsole();
    const store = mockEvidenceStore([row]);
    const { deps } = evidenceDeps({ evidenceStore: store });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync(["node", "maestro", "evidence", "show", "evd-0000000000010-de9ead"]);

    const lines = captured.logs;
    expect(lines.some((l) => l.includes("Gate: fail"))).toBe(true);
    expect(lines.some((l) => l.includes("feature_flag: ok"))).toBe(true);
    expect(lines.some((l) => l.includes("canary_plan: fail"))).toBe(true);
    expect(lines.some((l) => l.includes("rollback: ok"))).toBe(true);
    expect(lines.some((l) => l.includes("owner: fail"))).toBe(true);
  });

  it("renders runtime-signal with value, operator, threshold, pass, and sampled_at", async () => {
    const payload: RuntimeSignalPayload = {
      signal_name: "p99_latency",
      provider: "prometheus",
      query: "histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m]))",
      value: 0.312,
      threshold: 0.5,
      operator: "<",
      pass: true,
      sampled_at: "2026-05-10T14:23:00.000Z",
    };
    const row = makeEvidenceRow({
      id: "evd-0000000000011-51651a",
      kind: "runtime-signal",
      payload,
    });
    const captured = captureConsole();
    const store = mockEvidenceStore([row]);
    const { deps } = evidenceDeps({ evidenceStore: store });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync(["node", "maestro", "evidence", "show", "evd-0000000000011-51651a"]);

    const lines = captured.logs;
    expect(lines.some((l) => l.includes("Signal: p99_latency"))).toBe(true);
    expect(lines.some((l) => l.includes("Provider: prometheus"))).toBe(true);
    expect(lines.some((l) => l.includes("0.312") && l.includes("<") && l.includes("0.5"))).toBe(true);
    expect(lines.some((l) => l.includes("pass"))).toBe(true);
    expect(lines.some((l) => l.includes("2026-05-10T14:23:00.000Z"))).toBe(true);
  });

  it("renders runtime-signal note when present", async () => {
    const payload: RuntimeSignalPayload = {
      signal_name: "error_rate",
      provider: "datadog",
      query: "avg:http.request.errors",
      value: 0,
      threshold: 0.01,
      operator: "<",
      pass: false,
      sampled_at: "2026-05-10T14:23:00.000Z",
      note: "unsupported provider",
    };
    const row = makeEvidenceRow({
      id: "evd-0000000000012-1a2b3c",
      kind: "runtime-signal",
      payload,
    });
    const captured = captureConsole();
    const store = mockEvidenceStore([row]);
    const { deps } = evidenceDeps({ evidenceStore: store });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync(["node", "maestro", "evidence", "show", "evd-0000000000012-1a2b3c"]);

    const lines = captured.logs;
    expect(lines.some((l) => l.includes("Note: unsupported provider"))).toBe(true);
  });

  it("renders rollback-exercised with command and exit", async () => {
    const payload: RollbackExercisedPayload = {
      command: "./scripts/rollback.sh",
      exit: 0,
    };
    const row = makeEvidenceRow({
      id: "evd-0000000000013-b01bac",
      kind: "rollback-exercised",
      witness_level: "witnessed-by-ci",
      payload,
    });
    const captured = captureConsole();
    const store = mockEvidenceStore([row]);
    const { deps } = evidenceDeps({ evidenceStore: store });

    const program = makeProgram();
    registerEvidenceCommand(program, deps);
    await program.parseAsync(["node", "maestro", "evidence", "show", "evd-0000000000013-b01bac"]);

    const lines = captured.logs;
    expect(lines.some((l) => l.includes("Command: ./scripts/rollback.sh"))).toBe(true);
    expect(lines.some((l) => l.includes("Exit: 0"))).toBe(true);
  });
});
