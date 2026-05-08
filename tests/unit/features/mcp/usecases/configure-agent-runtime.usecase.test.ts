import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  buildMaestroAgentMcpConfigEntry,
  configureAgentRuntime,
  defaultAgentRuntimeTargets,
  entriesEqual,
  readClaudeUserScopeMaestro,
  readCodexMaestro,
  resolveMaestroBinaryInstallPath,
  type AgentRuntimeTarget,
  type RunCliResult,
} from "@/features/mcp/usecases/configure-agent-runtime.usecase.js";

describe("buildMaestroAgentMcpConfigEntry", () => {
  it("returns the binary path as command and ['mcp','serve'] as args", () => {
    const entry = buildMaestroAgentMcpConfigEntry("/abs/maestro");
    expect(entry.command).toBe("/abs/maestro");
    expect(entry.args).toEqual(["mcp", "serve"]);
  });
});

describe("resolveMaestroBinaryInstallPath", () => {
  it("uses 'maestro' on darwin/linux and 'maestro.exe' on win32", () => {
    expect(resolveMaestroBinaryInstallPath("/install", "darwin")).toBe("/install/maestro");
    expect(resolveMaestroBinaryInstallPath("/install", "linux")).toBe("/install/maestro");
    expect(resolveMaestroBinaryInstallPath("/install", "win32")).toBe("/install/maestro.exe");
  });
});

describe("defaultAgentRuntimeTargets", () => {
  it("targets the canonical agent config files (~/.claude.json, ~/.codex/config.toml)", () => {
    const targets = defaultAgentRuntimeTargets("/home/u");
    expect(targets).toHaveLength(2);
    expect(targets[0]).toMatchObject({
      name: "Claude Code",
      kind: "claude-code",
      cliBinary: "claude",
      configPath: "/home/u/.claude.json",
    });
    expect(targets[1]).toMatchObject({
      name: "Codex",
      kind: "codex",
      cliBinary: "codex",
      configPath: "/home/u/.codex/config.toml",
    });
  });
});

describe("entriesEqual", () => {
  it("compares command and args strictly", () => {
    expect(entriesEqual({ command: "/a", args: ["x"] }, { command: "/a", args: ["x"] })).toBe(true);
    expect(entriesEqual({ command: "/a", args: ["x"] }, { command: "/b", args: ["x"] })).toBe(false);
    expect(entriesEqual({ command: "/a", args: ["x"] }, { command: "/a", args: ["y"] })).toBe(false);
    expect(entriesEqual({ command: "/a", args: ["x"] }, { command: "/a", args: ["x", "y"] })).toBe(
      false,
    );
  });
});

describe("readClaudeUserScopeMaestro", () => {
  let root: string;
  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), "maestro-claude-"));
  });
  afterEach(() => {
    rmSync(root, { recursive: true, force: true });
  });

  it("returns undefined when the file does not exist", () => {
    expect(readClaudeUserScopeMaestro(join(root, "missing.json"))).toBeUndefined();
  });

  it("returns undefined when there is no maestro entry", () => {
    const path = join(root, ".claude.json");
    writeFileSync(path, JSON.stringify({ mcpServers: { other: { command: "/x" } } }));
    expect(readClaudeUserScopeMaestro(path)).toBeUndefined();
  });

  it("reads command and args from the top-level mcpServers.maestro entry", () => {
    const path = join(root, ".claude.json");
    writeFileSync(
      path,
      JSON.stringify({
        unrelated: { foo: "bar" },
        mcpServers: { maestro: { command: "/abs/maestro", args: ["mcp", "serve"] } },
      }),
    );
    expect(readClaudeUserScopeMaestro(path)).toEqual({
      command: "/abs/maestro",
      args: ["mcp", "serve"],
    });
  });

  it("treats malformed JSON as a thrown error", () => {
    const path = join(root, ".claude.json");
    writeFileSync(path, "{ not json");
    expect(() => readClaudeUserScopeMaestro(path)).toThrow();
  });
});

describe("readCodexMaestro", () => {
  let root: string;
  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), "maestro-codex-"));
  });
  afterEach(() => {
    rmSync(root, { recursive: true, force: true });
  });

  it("returns undefined when the file does not exist", () => {
    expect(readCodexMaestro(join(root, "missing.toml"))).toBeUndefined();
  });

  it("returns undefined when there is no [mcp_servers.maestro] table", () => {
    const path = join(root, "config.toml");
    writeFileSync(
      path,
      ['[mcp_servers.gitnexus]', 'command = "npx"', 'args = ["-y", "gitnexus@latest", "mcp"]', ""].join(
        "\n",
      ),
    );
    expect(readCodexMaestro(path)).toBeUndefined();
  });

  it("reads command and args from a [mcp_servers.maestro] table", () => {
    const path = join(root, "config.toml");
    writeFileSync(
      path,
      [
        'model = "gpt-5.5"',
        "",
        "[mcp_servers.maestro]",
        'command = "/abs/maestro"',
        'args = ["mcp", "serve"]',
        "",
        "[mcp_servers.gitnexus]",
        'command = "npx"',
        "",
      ].join("\n"),
    );
    expect(readCodexMaestro(path)).toEqual({
      command: "/abs/maestro",
      args: ["mcp", "serve"],
    });
  });

  it("stops at the next [...] heading and ignores later tables' command keys", () => {
    const path = join(root, "config.toml");
    writeFileSync(
      path,
      [
        "[mcp_servers.maestro]",
        'command = "/correct"',
        "[mcp_servers.other]",
        'command = "/wrong"',
        "",
      ].join("\n"),
    );
    expect(readCodexMaestro(path)?.command).toBe("/correct");
  });

  it("supports the quoted-key form [mcp_servers.\"maestro\"]", () => {
    const path = join(root, "config.toml");
    writeFileSync(
      path,
      ['[mcp_servers."maestro"]', 'command = "/abs/maestro"', 'args = ["mcp", "serve"]', ""].join(
        "\n",
      ),
    );
    expect(readCodexMaestro(path)).toEqual({
      command: "/abs/maestro",
      args: ["mcp", "serve"],
    });
  });

  it("strips inline comments after the value", () => {
    const path = join(root, "config.toml");
    writeFileSync(
      path,
      ["[mcp_servers.maestro]", 'command = "/abs/maestro" # set by maestro install', ""].join("\n"),
    );
    expect(readCodexMaestro(path)?.command).toBe("/abs/maestro");
  });
});

describe("configureAgentRuntime", () => {
  const claudeTarget: AgentRuntimeTarget = {
    name: "Claude Code",
    kind: "claude-code",
    cliBinary: "claude",
    configPath: "/will/not/read",
  };
  const codexTarget: AgentRuntimeTarget = {
    name: "Codex",
    kind: "codex",
    cliBinary: "codex",
    configPath: "/will/not/read",
  };
  const entry = { command: "/abs/maestro", args: ["mcp", "serve"] as const };

  let root: string;
  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), "maestro-cfgrt-"));
  });
  afterEach(() => {
    rmSync(root, { recursive: true, force: true });
  });

  it("returns 'skipped-no-runtime' when the CLI is not on PATH", () => {
    const r = configureAgentRuntime(claudeTarget, entry, {
      which: () => null,
      runCli: () => {
        throw new Error("should not be called");
      },
    });
    expect(r.action).toBe("skipped-no-runtime");
  });

  it("returns 'created' and runs `claude mcp add ... -s user` when no existing entry", () => {
    const path = join(root, ".claude.json");
    writeFileSync(path, JSON.stringify({}));
    const calls: string[][] = [];
    const r = configureAgentRuntime(
      { ...claudeTarget, configPath: path },
      entry,
      {
        which: () => "/usr/local/bin/claude",
        runCli: (argv) => {
          calls.push([...argv]);
          return { exitCode: 0, stderr: "" };
        },
      },
    );
    expect(r.action).toBe("created");
    expect(calls).toEqual([
      ["claude", "mcp", "add", "maestro", "-s", "user", "--", "/abs/maestro", "mcp", "serve"],
    ]);
  });

  it("returns 'unchanged' and does not run any CLI when the existing entry matches", () => {
    const path = join(root, ".claude.json");
    writeFileSync(
      path,
      JSON.stringify({
        mcpServers: { maestro: { command: "/abs/maestro", args: ["mcp", "serve"] } },
      }),
    );
    const calls: string[][] = [];
    const r = configureAgentRuntime(
      { ...claudeTarget, configPath: path },
      entry,
      {
        which: () => "/usr/local/bin/claude",
        runCli: (argv) => {
          calls.push([...argv]);
          return { exitCode: 0, stderr: "" };
        },
      },
    );
    expect(r.action).toBe("unchanged");
    expect(calls).toEqual([]);
  });

  it("returns 'updated' and runs remove + add when the existing entry drifts", () => {
    const path = join(root, ".claude.json");
    writeFileSync(
      path,
      JSON.stringify({ mcpServers: { maestro: { command: "/old/maestro", args: ["mcp", "serve"] } } }),
    );
    const calls: string[][] = [];
    const r = configureAgentRuntime(
      { ...claudeTarget, configPath: path },
      entry,
      {
        which: () => "/usr/local/bin/claude",
        runCli: (argv) => {
          calls.push([...argv]);
          return { exitCode: 0, stderr: "" };
        },
      },
    );
    expect(r.action).toBe("updated");
    expect(calls).toEqual([
      ["claude", "mcp", "remove", "maestro", "-s", "user"],
      ["claude", "mcp", "add", "maestro", "-s", "user", "--", "/abs/maestro", "mcp", "serve"],
    ]);
  });

  it("uses the codex argv shape (no -s flag) when targeting Codex", () => {
    const path = join(root, "config.toml");
    writeFileSync(path, "");
    const calls: string[][] = [];
    const r = configureAgentRuntime(
      { ...codexTarget, configPath: path },
      entry,
      {
        which: () => "/usr/local/bin/codex",
        runCli: (argv) => {
          calls.push([...argv]);
          return { exitCode: 0, stderr: "" };
        },
      },
    );
    expect(r.action).toBe("created");
    expect(calls).toEqual([
      ["codex", "mcp", "add", "maestro", "--", "/abs/maestro", "mcp", "serve"],
    ]);
  });

  it("returns 'error' with the CLI stderr when add fails", () => {
    const path = join(root, ".claude.json");
    writeFileSync(path, JSON.stringify({}));
    const r = configureAgentRuntime(
      { ...claudeTarget, configPath: path },
      entry,
      {
        which: () => "/usr/local/bin/claude",
        runCli: (): RunCliResult => ({ exitCode: 1, stderr: "boom" }),
      },
    );
    expect(r.action).toBe("error");
    expect(r.error).toMatch(/exited 1: boom/);
  });

  it("returns 'error' when the existing config file is malformed JSON", () => {
    const path = join(root, ".claude.json");
    writeFileSync(path, "{ not json");
    const r = configureAgentRuntime(
      { ...claudeTarget, configPath: path },
      entry,
      {
        which: () => "/usr/local/bin/claude",
        runCli: () => {
          throw new Error("should not be called");
        },
      },
    );
    expect(r.action).toBe("error");
    expect(typeof r.error).toBe("string");
  });
});

describe("install-script integration: parent dir auto-creation no longer required", () => {
  it("Codex target points at ~/.codex/config.toml even when parent dir is absent (codex CLI handles creation)", () => {
    const targets = defaultAgentRuntimeTargets("/nonexistent");
    const codex = targets.find((t) => t.kind === "codex");
    expect(codex?.configPath).toBe("/nonexistent/.codex/config.toml");
  });

  it("supports caller-supplied dummy home for tests by passing via defaultAgentRuntimeTargets(home)", () => {
    const tmpHome = mkdtempSync(join(tmpdir(), "maestro-home-"));
    mkdirSync(join(tmpHome, ".codex"));
    const targets = defaultAgentRuntimeTargets(tmpHome);
    expect(targets[1]?.configPath).toBe(join(tmpHome, ".codex", "config.toml"));
    rmSync(tmpHome, { recursive: true, force: true });
  });
});
