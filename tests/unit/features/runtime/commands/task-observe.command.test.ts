import { afterEach, beforeEach, describe, expect, it, mock } from "bun:test";
import { Command } from "commander";
import { registerTaskObserveCommand } from "@/features/runtime/commands/task-observe.command.js";
import type { DevObservabilityPort } from "@/features/runtime/ports/dev-observability.port.js";

interface RecordedEvidenceCall {
  readonly task_id: string;
  readonly kind: string;
  readonly note: string;
}

function stubMetrics(result: { value: number; source?: string; sampledAt?: string } | Error): DevObservabilityPort {
  return {
    queryMetric: async () => {
      if (result instanceof Error) throw result;
      return {
        value: result.value,
        source: result.source ?? "prometheus@stub",
        sampledAt: result.sampledAt ?? "2026-05-16T00:00:00.000Z",
      };
    },
    tailLogs: async () => {
      throw new Error("stubMetrics.tailLogs not supported");
    },
  };
}

function stubLogs(
  result:
    | { lines: { text: string }[]; source?: string }
    | Error,
  forwardCalls?: { filter?: string; lines?: number }[],
): DevObservabilityPort {
  return {
    queryMetric: async () => {
      throw new Error("stubLogs.queryMetric not supported");
    },
    tailLogs: async (filter, lines) => {
      if (forwardCalls !== undefined) {
        forwardCalls.push({
          ...(filter !== undefined ? { filter } : {}),
          ...(lines !== undefined ? { lines } : {}),
        });
      }
      if (result instanceof Error) throw result;
      return {
        lines: result.lines,
        source: result.source ?? "file:/tmp/app.log",
      };
    },
  };
}

function makeProgram(args: {
  buildPrometheusAdapter?: (baseUrl: string) => DevObservabilityPort;
  buildLogTailAdapter?: (filePath?: string) => DevObservabilityPort;
  recordedCalls?: RecordedEvidenceCall[];
  env?: NodeJS.ProcessEnv;
}): Command {
  const program = new Command().exitOverride();
  const task = program.command("task").description("Task commands");

  registerTaskObserveCommand(task, {
    ...(args.buildPrometheusAdapter ? { buildPrometheusAdapter: args.buildPrometheusAdapter } : {}),
    ...(args.buildLogTailAdapter ? { buildLogTailAdapter: args.buildLogTailAdapter } : {}),
    resolveRepoRoot: () => "/tmp",
    readEnv: () => args.env ?? {},
    getEvidenceStore: () => ({ append: async () => {}, list: async () => [] }) as never,
    recordEvidence: (async (
      _store: unknown,
      input: { task_id: string; kind: string; payload: { note?: string } },
    ) => {
      args.recordedCalls?.push({
        task_id: input.task_id,
        kind: input.kind,
        note: input.payload.note ?? "",
      });
      return {} as never;
    }) as never,
  });

  return program;
}

interface ConsoleSpies {
  logs: string[];
  errs: string[];
  restore: () => void;
}

function spyConsole(): ConsoleSpies {
  const logs: string[] = [];
  const errs: string[] = [];
  const origLog = console.log;
  const origErr = console.error;
  console.log = (...args: unknown[]) => {
    logs.push(args.map((a) => (typeof a === "string" ? a : JSON.stringify(a))).join(" "));
  };
  console.error = (...args: unknown[]) => {
    errs.push(args.map((a) => (typeof a === "string" ? a : JSON.stringify(a))).join(" "));
  };
  return {
    logs,
    errs,
    restore: () => {
      console.log = origLog;
      console.error = origErr;
    },
  };
}

describe("task observe metrics", () => {
  let spies: ConsoleSpies;
  beforeEach(() => {
    spies = spyConsole();
    process.exitCode = 0;
  });
  afterEach(() => {
    spies.restore();
    process.exitCode = 0;
  });

  it("prints value on the happy path with --prometheus-url", async () => {
    const program = makeProgram({
      buildPrometheusAdapter: () => stubMetrics({ value: 42.5 }),
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "metrics", "up",
      "--prometheus-url", "http://stub:9090",
    ]);
    expect(spies.logs.join("\n")).toContain("value=42.5");
    expect(process.exitCode === 0 || process.exitCode === undefined).toBe(true);
  });

  it("emits a JSON envelope under --json", async () => {
    const program = makeProgram({
      buildPrometheusAdapter: () => stubMetrics({ value: 7, source: "src-x", sampledAt: "2026-05-16T00:00:00.000Z" }),
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "metrics", "up",
      "--prometheus-url", "http://stub:9090", "--json",
    ]);
    const body = JSON.parse(spies.logs.join(""));
    expect(body.kind).toBe("metrics");
    expect(body.value).toBe(7);
    expect(body.source).toBe("src-x");
  });

  it("exits 2 when the adapter throws", async () => {
    const program = makeProgram({
      buildPrometheusAdapter: () => stubMetrics(new Error("prometheus: HTTP 502")),
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "metrics", "up",
      "--prometheus-url", "http://stub:9090",
    ]);
    expect(process.exitCode).toBe(2);
    expect(spies.errs.join("\n")).toContain("HTTP 502");
  });

  it("exits 1 when no URL is configured", async () => {
    const program = makeProgram({
      buildPrometheusAdapter: () => stubMetrics({ value: 0 }),
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "metrics", "up",
    ]);
    expect(process.exitCode).toBe(1);
    expect(spies.errs.join("\n")).toContain("MAESTRO_PROMETHEUS_URL");
  });

  it("falls back to MAESTRO_PROMETHEUS_URL when no flag is passed", async () => {
    const program = makeProgram({
      buildPrometheusAdapter: () => stubMetrics({ value: 1 }),
      env: { MAESTRO_PROMETHEUS_URL: "http://env:9090" },
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "metrics", "up",
    ]);
    expect(process.exitCode === 0 || process.exitCode === undefined).toBe(true);
    expect(spies.logs.join("\n")).toContain("value=1");
  });

  it("records a manual-note evidence row tagged dev-observation:metrics with --record --task", async () => {
    const recorded: RecordedEvidenceCall[] = [];
    const program = makeProgram({
      buildPrometheusAdapter: () => stubMetrics({ value: 3.14 }),
      recordedCalls: recorded,
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "metrics", "rate(errors[5m])",
      "--prometheus-url", "http://stub:9090",
      "--record", "--task", "tsk-abc-123",
    ]);
    expect(process.exitCode === 0 || process.exitCode === undefined).toBe(true);
    expect(recorded).toHaveLength(1);
    expect(recorded[0]!.kind).toBe("manual-note");
    expect(recorded[0]!.task_id).toBe("tsk-abc-123");
    expect(recorded[0]!.note).toContain("[dev-observation:metrics]");
    expect(recorded[0]!.note).toContain("value=3.14");
  });

  it("exits 1 when --record is set without --task", async () => {
    const program = makeProgram({
      buildPrometheusAdapter: () => stubMetrics({ value: 1 }),
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "metrics", "up",
      "--prometheus-url", "http://stub:9090", "--record",
    ]);
    expect(process.exitCode).toBe(1);
    expect(spies.errs.join("\n")).toContain("--task");
  });
});

describe("task observe logs", () => {
  let spies: ConsoleSpies;
  beforeEach(() => {
    spies = spyConsole();
    process.exitCode = 0;
  });
  afterEach(() => {
    spies.restore();
    process.exitCode = 0;
  });

  it("prints lines on the happy path", async () => {
    const program = makeProgram({
      buildLogTailAdapter: () => stubLogs({ lines: [{ text: "line-A" }, { text: "line-B" }] }),
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "logs",
      "--log-file", "/tmp/x.log",
    ]);
    const joined = spies.logs.join("\n");
    expect(joined).toContain("line-A");
    expect(joined).toContain("line-B");
    expect(process.exitCode === 0 || process.exitCode === undefined).toBe(true);
  });

  it("emits a JSON envelope under --json", async () => {
    const program = makeProgram({
      buildLogTailAdapter: () => stubLogs({ lines: [{ text: "x" }], source: "file:/tmp/y.log" }),
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "logs",
      "--log-file", "/tmp/y.log", "--json",
    ]);
    const body = JSON.parse(spies.logs.join(""));
    expect(body.kind).toBe("logs");
    expect(body.source).toBe("file:/tmp/y.log");
    expect(body.lines).toEqual([{ text: "x" }]);
  });

  it("threads --filter and --lines to the adapter", async () => {
    const calls: { filter?: string; lines?: number }[] = [];
    const program = makeProgram({
      buildLogTailAdapter: () => stubLogs({ lines: [] }, calls),
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "logs",
      "--log-file", "/tmp/z.log", "--filter", "error", "--lines", "5",
    ]);
    expect(calls).toHaveLength(1);
    expect(calls[0]!.filter).toBe("error");
    expect(calls[0]!.lines).toBe(5);
  });

  it("exits 2 when adapter tailLogs throws", async () => {
    const program = makeProgram({
      buildLogTailAdapter: () => stubLogs(new Error("ENOENT")),
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "logs",
      "--log-file", "/tmp/missing.log",
    ]);
    expect(process.exitCode).toBe(2);
    expect(spies.errs.join("\n")).toContain("ENOENT");
  });

  it("exits 1 when adapter ctor throws because no path is configured", async () => {
    const program = makeProgram({
      buildLogTailAdapter: () => {
        throw new Error("log-tail: no path; pass --log-file or set MAESTRO_DEV_LOG_FILE");
      },
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "logs",
    ]);
    expect(process.exitCode).toBe(1);
    expect(spies.errs.join("\n")).toContain("MAESTRO_DEV_LOG_FILE");
  });

  it("exits 1 when --lines is not a positive integer", async () => {
    const program = makeProgram({
      buildLogTailAdapter: () => stubLogs({ lines: [] }),
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "logs",
      "--log-file", "/tmp/x.log", "--lines", "0",
    ]);
    expect(process.exitCode).toBe(1);
    expect(spies.errs.join("\n")).toContain("--lines");
  });

  it("records a manual-note evidence row tagged dev-observation:logs with --record --task", async () => {
    const recorded: RecordedEvidenceCall[] = [];
    const program = makeProgram({
      buildLogTailAdapter: () => stubLogs({ lines: [{ text: "row1" }, { text: "row2" }] }),
      recordedCalls: recorded,
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "logs",
      "--log-file", "/tmp/x.log",
      "--record", "--task", "tsk-log-1",
    ]);
    expect(process.exitCode === 0 || process.exitCode === undefined).toBe(true);
    expect(recorded).toHaveLength(1);
    expect(recorded[0]!.kind).toBe("manual-note");
    expect(recorded[0]!.task_id).toBe("tsk-log-1");
    expect(recorded[0]!.note).toContain("[dev-observation:logs]");
    expect(recorded[0]!.note).toContain("lines=2");
  });

  it("exits 1 when --record is set without --task", async () => {
    const program = makeProgram({
      buildLogTailAdapter: () => stubLogs({ lines: [] }),
    });
    await program.parseAsync([
      "node", "maestro", "task", "observe", "logs",
      "--log-file", "/tmp/x.log", "--record",
    ]);
    expect(process.exitCode).toBe(1);
    expect(spies.errs.join("\n")).toContain("--task");
  });
});

void mock;
