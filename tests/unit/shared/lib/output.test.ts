import { afterEach, describe, expect, it } from "bun:test";
import {
  formatAgentResults,
  output,
  resolveJsonFlag,
  warn,
} from "@/shared/lib/output.js";

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

describe("output", () => {
  it("prints formatted text lines after terminal sanitization", () => {
    const captured = captureConsole();

    output(false, { message: "hello" }, () => [
      "line 1",
      "line 2\u001b[31m alert\u001b[0m",
    ]);

    expect(captured.logs).toEqual([
      "line 1",
      "line 2 alert",
    ]);
    expect(captured.errors).toEqual([]);
  });

  it("prints JSON payloads without calling the formatter", () => {
    const captured = captureConsole();
    let formatterCalls = 0;

    output(true, { ok: true }, () => {
      formatterCalls += 1;
      return ["unused"];
    });

    expect(captured.logs).toEqual([
      JSON.stringify({ ok: true }, null, 2),
    ]);
    expect(formatterCalls).toBe(0);
  });
});

describe("resolveJsonFlag", () => {
  it("prefers the leaf json option when present", () => {
    expect(resolveJsonFlag(
      { json: true, jsonGroup: false },
      { opts: () => ({ json: false }) },
    )).toBe(true);
  });

  it("falls back to the group json option", () => {
    expect(resolveJsonFlag(
      { jsonGroup: true },
      { opts: () => ({ json: false }) },
    )).toBe(true);
  });

  it("falls back to the root program json option", () => {
    expect(resolveJsonFlag(
      {},
      { opts: () => ({ json: true }) },
    )).toBe(true);
  });

  it("defaults to false when no json flag is set anywhere", () => {
    expect(resolveJsonFlag(
      {},
      { opts: () => ({}) },
    )).toBe(false);
  });
});

describe("warn", () => {
  it("writes warnings to stderr", () => {
    const captured = captureConsole();

    warn("careful");

    expect(captured.errors).toEqual(["[!] careful"]);
    expect(captured.logs).toEqual([]);
  });
});

describe("formatAgentResults", () => {
  it("formats each agent result on its own line", () => {
    expect(formatAgentResults([
      {
        agent: "claude",
        action: "installed",
        configPath: "/tmp/claude.md",
      },
      {
        agent: "codex",
        action: "removed",
        configPath: "/tmp/agents.md",
      },
    ])).toEqual([
      "  claude: installed (/tmp/claude.md)",
      "  codex: removed (/tmp/agents.md)",
    ]);
  });

  it("appends an install hint when every agent is not-detected", () => {
    const lines = formatAgentResults([
      { agent: "Claude Code", action: "not-detected", configPath: "/tmp/.claude" },
      { agent: "Codex", action: "not-detected", configPath: "/tmp/.codex" },
    ]);
    expect(lines).toContain("  Claude Code: not-detected (/tmp/.claude)");
    expect(lines.some((line) => line.includes("No supported agents detected"))).toBe(true);
  });

  it("does not append the hint when at least one agent is detected", () => {
    const lines = formatAgentResults([
      { agent: "Claude Code", action: "installed", configPath: "/tmp/.claude" },
      { agent: "Codex", action: "not-detected", configPath: "/tmp/.codex" },
    ]);
    expect(lines.some((line) => line.includes("No supported agents detected"))).toBe(false);
  });
});
