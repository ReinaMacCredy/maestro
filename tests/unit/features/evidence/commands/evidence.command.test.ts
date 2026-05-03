import { afterEach, describe, expect, it } from "bun:test";
import { Command } from "commander";
import {
  registerEvidenceCommand,
  recordEvidence as realRecordEvidence,
  type EvidenceRow,
  type EvidenceStorePort,
  type RecordEvidenceInput,
} from "@/features/evidence";
import { mockEvidenceStore } from "../../../../helpers/mocks.js";
import type { Task } from "@/features/task";
import type { TaskStorePort } from "@/features/task/ports/task-store.port.js";
import type { AgentSession, SessionDetectPort } from "@/features/session";

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

function fakeSessionDetect(session?: AgentSession): Pick<SessionDetectPort, "detect" | "lookup"> {
  return {
    detect: async () => session,
    lookup: async () => undefined,
  };
}

interface DepsOverrides {
  readonly tasks?: readonly Task[];
  readonly session?: AgentSession;
  readonly noSession?: boolean;
  readonly evidenceStore?: EvidenceStorePort;
  readonly recordEvidence?: typeof realRecordEvidence;
}

function evidenceDeps(overrides: DepsOverrides = {}) {
  const session = overrides.noSession
    ? undefined
    : overrides.session ?? {
        agent: "claude-code",
        sessionId: "sess-test",
        sourcePath: "/tmp/sess-test",
      };
  const tasks = overrides.tasks ?? [makeTask("tsk-aaaaaa")];
  const evidenceStore = overrides.evidenceStore ?? mockEvidenceStore();
  return {
    deps: {
      getServices: () => ({
        evidenceStore,
        taskStore: fakeTaskStore(tasks) as TaskStorePort,
        sessionDetect: fakeSessionDetect(session) as SessionDetectPort,
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
  it("records command-kind evidence with detected session and witness level agent-claimed-locally", async () => {
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
    expect(received!.session_id).toBe("sess-test");
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

  it("--session overrides the detected session", async () => {
    captureConsole();
    let received: RecordEvidenceInput | undefined;
    const { deps } = evidenceDeps({
      session: {
        agent: "claude-code",
        sessionId: "sess-detected",
        sourcePath: "/tmp/sess-detected",
      },
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

  it("omits session_id when no session is detected and none provided", async () => {
    captureConsole();
    let received: RecordEvidenceInput | undefined;
    const { deps } = evidenceDeps({
      noSession: true,
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
