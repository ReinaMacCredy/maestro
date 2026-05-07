import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  buildMaestroAgentMcpConfigEntry,
  configureAgentRuntime,
  defaultAgentRuntimeTargets,
  resolveMaestroBinaryInstallPath,
} from "@/features/mcp/usecases/configure-agent-runtime.usecase.js";

describe("buildMaestroAgentMcpConfigEntry", () => {
  it("returns the binary path as command and ['mcp','serve'] as args", () => {
    const entry = buildMaestroAgentMcpConfigEntry("/abs/maestro");
    expect(entry.command).toBe("/abs/maestro");
    expect(entry.args).toEqual(["mcp", "serve"]);
  });
});

describe("resolveMaestroBinaryInstallPath", () => {
  it("uses 'maestro' on darwin/linux", () => {
    expect(resolveMaestroBinaryInstallPath("/install", "darwin")).toBe("/install/maestro");
    expect(resolveMaestroBinaryInstallPath("/install", "linux")).toBe("/install/maestro");
  });

  it("uses 'maestro.exe' on win32", () => {
    expect(resolveMaestroBinaryInstallPath("/install", "win32")).toBe("/install/maestro.exe");
  });
});

describe("defaultAgentRuntimeTargets", () => {
  it("returns Claude Code and Codex paths under the given home", () => {
    const targets = defaultAgentRuntimeTargets("/home/u");
    expect(targets).toHaveLength(2);
    expect(targets[0].name).toBe("Claude Code");
    expect(targets[0].configPath).toBe("/home/u/.claude/mcp.json");
    expect(targets[1].name).toBe("Codex");
    expect(targets[1].configPath).toBe("/home/u/.codex/mcp.json");
  });
});

describe("configureAgentRuntime", () => {
  let root: string;

  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), "maestro-mcp-cfg-"));
  });

  afterEach(() => {
    rmSync(root, { recursive: true, force: true });
  });

  it("returns 'skipped-no-runtime' when the parent dir is absent and createIfMissing=false", () => {
    const target = { name: "X", configPath: join(root, ".missing", "mcp.json") };
    const r = configureAgentRuntime(target, buildMaestroAgentMcpConfigEntry("/abs/maestro"), {
      createIfMissing: false,
    });
    expect(r.action).toBe("skipped-no-runtime");
    expect(existsSync(target.configPath)).toBe(false);
  });

  it("creates a fresh config when the parent dir exists and the file does not", () => {
    const parent = join(root, ".claude");
    mkdirSync(parent);
    const target = { name: "Claude Code", configPath: join(parent, "mcp.json") };
    const entry = buildMaestroAgentMcpConfigEntry("/abs/maestro");
    const r = configureAgentRuntime(target, entry, { createIfMissing: false });
    expect(r.action).toBe("created");
    const written = JSON.parse(readFileSync(target.configPath, "utf8"));
    expect(written.mcpServers.maestro.command).toBe("/abs/maestro");
    expect(written.mcpServers.maestro.args).toEqual(["mcp", "serve"]);
  });

  it("creates the parent dir when createIfMissing=true (default) and writes the file", () => {
    const target = { name: "Codex", configPath: join(root, "fresh", ".codex", "mcp.json") };
    const r = configureAgentRuntime(target, buildMaestroAgentMcpConfigEntry("/abs/maestro"));
    expect(r.action).toBe("created");
    expect(existsSync(target.configPath)).toBe(true);
  });

  it("returns 'unchanged' when the existing entry already matches", () => {
    const parent = join(root, ".claude");
    mkdirSync(parent);
    const target = { name: "Claude Code", configPath: join(parent, "mcp.json") };
    writeFileSync(
      target.configPath,
      JSON.stringify(
        { mcpServers: { maestro: { command: "/abs/maestro", args: ["mcp", "serve"] } } },
        null,
        2,
      ),
    );
    const r = configureAgentRuntime(target, buildMaestroAgentMcpConfigEntry("/abs/maestro"), {
      createIfMissing: false,
    });
    expect(r.action).toBe("unchanged");
  });

  it("updates command/args when an existing entry differs and preserves unrelated keys", () => {
    const parent = join(root, ".claude");
    mkdirSync(parent);
    const target = { name: "Claude Code", configPath: join(parent, "mcp.json") };
    writeFileSync(
      target.configPath,
      JSON.stringify(
        {
          someUnrelated: "keep me",
          mcpServers: {
            maestro: { command: "/old/path", args: ["serve"] },
            other: { command: "/other", args: [] },
          },
        },
        null,
        2,
      ),
    );
    const r = configureAgentRuntime(target, buildMaestroAgentMcpConfigEntry("/abs/maestro"), {
      createIfMissing: false,
    });
    expect(r.action).toBe("updated");
    const written = JSON.parse(readFileSync(target.configPath, "utf8"));
    expect(written.someUnrelated).toBe("keep me");
    expect(written.mcpServers.maestro.command).toBe("/abs/maestro");
    expect(written.mcpServers.maestro.args).toEqual(["mcp", "serve"]);
    expect(written.mcpServers.other).toEqual({ command: "/other", args: [] });
  });

  it("returns 'error' when the existing config is malformed JSON", () => {
    const parent = join(root, ".claude");
    mkdirSync(parent);
    const target = { name: "Claude Code", configPath: join(parent, "mcp.json") };
    writeFileSync(target.configPath, "{ not json");
    const r = configureAgentRuntime(target, buildMaestroAgentMcpConfigEntry("/abs/maestro"), {
      createIfMissing: false,
    });
    expect(r.action).toBe("error");
    expect(typeof r.error).toBe("string");
  });

  it("treats junk mcpServers value as no prior maestro entry and creates fresh", () => {
    const parent = join(root, ".claude");
    mkdirSync(parent);
    const target = { name: "Claude Code", configPath: join(parent, "mcp.json") };
    writeFileSync(target.configPath, JSON.stringify({ mcpServers: ["not", "an", "object"] }));
    const r = configureAgentRuntime(target, buildMaestroAgentMcpConfigEntry("/abs/maestro"), {
      createIfMissing: false,
    });
    expect(r.action).toBe("created");
    const written = JSON.parse(readFileSync(target.configPath, "utf8"));
    expect(written.mcpServers.maestro.command).toBe("/abs/maestro");
    expect(written.mcpServers.maestro.args).toEqual(["mcp", "serve"]);
  });
});
