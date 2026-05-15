import { afterEach, beforeEach, describe, expect, it, mock } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { Command } from "commander";

const originalConsoleLog = console.log;
const originalConsoleError = console.error;

let tmpDir: string;

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

async function loadRegisterReplyCommand(replyStore: {
  write: (reply: Record<string, unknown>) => Promise<void>;
  list: () => Promise<readonly Record<string, unknown>[]>;
}) {
  const mod = await import("@/features/reply/commands/reply.command.js");
  const deps = { getServices: () => ({ replyStore } as never) };
  return {
    registerReplyCommand: (program: Command): void => mod.registerReplyCommand(program, deps),
  };
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-reply-command-unit-"));
  await mkdir(join(tmpDir, ".maestro"), { recursive: true });
});

afterEach(async () => {
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
  mock.restore();
  await rm(tmpDir, { recursive: true, force: true });
});

describe("registerReplyCommand", () => {
  it("writes a reply in json mode", async () => {
    const captured = captureConsole();
    const writes: Record<string, unknown>[] = [];
    const reportFile = join(tmpDir, "report.json");
    await writeFile(reportFile, JSON.stringify({ content: "Completed the task" }));

    const { registerReplyCommand } = await loadRegisterReplyCommand({
      write: async (reply) => {
        writes.push(reply);
      },
      list: async () => [],
    });

    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerReplyCommand(program);

    await program.parseAsync([
      "node",
      "maestro",
      "reply",
      "write",
      "f-42",
      "--mission",
      "2026-04-15-001",
      "--outcome",
      "completed",
      "--note",
      "done",
      "--source",
      "unit:test",
      "--agent",
      "--report-file",
      reportFile,
      "--json",
    ]);

    expect(writes).toHaveLength(1);
    expect(writes[0]).toMatchObject({
      missionId: "2026-04-15-001",
      featureId: "f-42",
      outcome: "completed",
      notes: "done",
      writtenBy: "agent",
      source: "unit:test",
      report: {
        salientSummary: "Completed the task",
      },
    });
    expect(JSON.parse(captured.logs[0] ?? "")).toMatchObject({
      missionId: "2026-04-15-001",
      featureId: "f-42",
      outcome: "completed",
    });
  });

  it("lists replies in text mode and reports the empty state", async () => {
    const captured = captureConsole();
    let listedOnce = false;

    const { registerReplyCommand } = await loadRegisterReplyCommand({
      write: async () => undefined,
      list: async () => {
        if (!listedOnce) {
          listedOnce = true;
          return [
            {
              missionId: "2026-04-15-001",
              featureId: "f-42",
              outcome: "completed",
              writtenAt: "2026-04-15T09:00:00.000Z",
              writtenBy: "human",
              notes: "line[31m alert[0m",
            },
          ];
        }

        return [];
      },
    });

    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerReplyCommand(program);

    await program.parseAsync(["node", "maestro", "reply", "list"]);
    expect(captured.logs).toContain("2026-04-15-001/f-42 [completed] 2026-04-15T09:00:00.000Z by human");
    expect(captured.logs).toContain("  note: line alert");

    captured.logs.length = 0;

    await program.parseAsync(["node", "maestro", "reply", "list"]);
    expect(captured.logs).toEqual(["(no replies on disk)"]);
  });

  it("rejects invalid outcomes", async () => {
    const { registerReplyCommand } = await loadRegisterReplyCommand({
      write: async () => undefined,
      list: async () => [],
    });

    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerReplyCommand(program);

    await expect(
      program.parseAsync([
        "node",
        "maestro",
        "reply",
        "write",
        "f-42",
        "--mission",
        "2026-04-15-001",
        "--outcome",
        "wrong",
      ]),
    ).rejects.toMatchObject({
      message: "--outcome is required (completed|kicked-back|abandoned)",
    });
  });

  it("rejects unreadable report files", async () => {
    const { registerReplyCommand } = await loadRegisterReplyCommand({
      write: async () => undefined,
      list: async () => [],
    });

    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerReplyCommand(program);

    await expect(
      program.parseAsync([
        "node",
        "maestro",
        "reply",
        "write",
        "f-42",
        "--mission",
        "2026-04-15-001",
        "--outcome",
        "completed",
        "--report-file",
        join(tmpDir, "missing.json"),
      ]),
    ).rejects.toMatchObject({
      message: expect.stringContaining("Cannot read --report-file"),
    });
  });

  it("rejects malformed report files", async () => {
    const reportFile = join(tmpDir, "broken.json");
    await writeFile(reportFile, "{bad json");
    const { registerReplyCommand } = await loadRegisterReplyCommand({
      write: async () => undefined,
      list: async () => [],
    });

    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerReplyCommand(program);

    await expect(
      program.parseAsync([
        "node",
        "maestro",
        "reply",
        "write",
        "f-42",
        "--mission",
        "2026-04-15-001",
        "--outcome",
        "completed",
        "--report-file",
        reportFile,
      ]),
    ).rejects.toMatchObject({
      message: expect.stringContaining("--report-file is not valid JSON"),
    });
  });
});
