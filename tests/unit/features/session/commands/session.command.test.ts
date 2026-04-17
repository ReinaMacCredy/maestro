import { afterEach, describe, expect, it, mock } from "bun:test";
import { Command } from "commander";
import { tmpdir } from "node:os";
import { join } from "node:path";

const SESSION_SOURCE = join(tmpdir(), "claude", "session.jsonl");
const CODEX_SOURCE = join(tmpdir(), "codex", "session.jsonl");

const originalConsoleLog = console.log;
const originalConsoleError = console.error;
const originalProcessExit = process.exit;

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

async function loadRegisterSessionCommand(sessionDetect: {
  detect: (cwd: string) => Promise<unknown>;
}) {
  mock.module("@/services.js", () => ({
    getServices: () => ({ sessionDetect }),
  }));

  return import(`@/features/session/commands/session.command.ts?test=${Date.now()}-${Math.random()}`);
}

afterEach(() => {
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
  process.exit = originalProcessExit;
  mock.restore();
});

describe("registerSessionCommand", () => {
  it("prints structured json output when a session is detected", async () => {
    const captured = captureConsole();
    const { registerSessionCommand } = await loadRegisterSessionCommand({
      detect: async (cwd: string) => ({
        agent: "codex",
        sessionId: "thread-123",
        sourcePath: `${cwd}/rollout.jsonl`,
        startedAt: 1_777_000_000_000,
      }),
    });

    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerSessionCommand(program);

    await program.parseAsync(["node", "maestro", "session", "--json"]);

    expect(JSON.parse(captured.logs[0] ?? "")).toEqual({
      session: {
        agent: "codex",
        sessionId: "thread-123",
        sourcePath: `${process.cwd()}/rollout.jsonl`,
        startedAt: 1_777_000_000_000,
      },
    });
  });

  it("prints human-readable text when a session is detected", async () => {
    const captured = captureConsole();
    const { registerSessionCommand } = await loadRegisterSessionCommand({
      detect: async () => ({
        agent: "claude-code",
        sessionId: "claude-1",
        sourcePath: SESSION_SOURCE,
        startedAt: 1_777_000_000_000,
      }),
    });

    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerSessionCommand(program);

    await program.parseAsync(["node", "maestro", "session"]);

    expect(captured.logs).toContain("Agent:     claude-code");
    expect(captured.logs).toContain("Session:   claude-1");
    expect(captured.logs).toContain(`Source:    ${SESSION_SOURCE}`);
    expect(captured.logs.some((line) => line.startsWith("Started:   "))).toBe(true);
  });

  it("prints only the session id in quiet mode", async () => {
    const captured = captureConsole();
    const { registerSessionCommand } = await loadRegisterSessionCommand({
      detect: async () => ({
        agent: "codex",
        sessionId: "quiet-123",
        sourcePath: CODEX_SOURCE,
      }),
    });

    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerSessionCommand(program);

    await program.parseAsync(["node", "maestro", "session", "--quiet"]);

    expect(captured.logs).toEqual(["quiet-123"]);
    expect(captured.errors).toEqual([]);
  });

  it("throws a MaestroError when no session is detected in normal mode", async () => {
    const captured = captureConsole();
    const { registerSessionCommand } = await loadRegisterSessionCommand({
      detect: async () => undefined,
    });

    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerSessionCommand(program);

    await expect(
      program.parseAsync(["node", "maestro", "session"]),
    ).rejects.toMatchObject({
      message: "No session detected",
    });

    expect(captured.logs).toEqual([]);
  });

  it("exits silently in quiet mode when no session is detected", async () => {
    const captured = captureConsole();
    const { registerSessionCommand } = await loadRegisterSessionCommand({
      detect: async () => undefined,
    });

    process.exit = ((code?: number) => {
      throw new Error(`exit:${code ?? 0}`);
    }) as typeof process.exit;

    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerSessionCommand(program);

    await expect(
      program.parseAsync(["node", "maestro", "session", "--quiet"]),
    ).rejects.toThrow("exit:1");

    expect(captured.logs).toEqual([]);
    expect(captured.errors).toEqual([]);
  });
});
