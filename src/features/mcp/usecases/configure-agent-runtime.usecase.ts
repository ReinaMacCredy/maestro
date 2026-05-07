import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";

export interface AgentMcpEntry {
  readonly command: string;
  readonly args: readonly string[];
  readonly env?: Record<string, string>;
}

export interface AgentRuntimeTarget {
  readonly name: string;
  readonly configPath: string;
}

export interface ConfigureRuntimeResult {
  readonly target: AgentRuntimeTarget;
  readonly action: "skipped-no-runtime" | "created" | "updated" | "unchanged" | "error";
  readonly error?: string;
}

const SERVER_KEY = "maestro";

/**
 * Path to the standalone Node entry point. Kept as a fallback for
 * environments that cannot launch the Bun-compiled binary directly.
 */
export function resolveStartMjsInstallPath(installDir: string): string {
  return join(installDir, "start.mjs");
}

export function resolveMaestroBinaryInstallPath(
  installDir: string,
  platform: NodeJS.Platform = process.platform,
): string {
  return join(installDir, platform === "win32" ? "maestro.exe" : "maestro");
}

/**
 * Build the agent-runtime MCP config entry. The compiled `maestro` binary
 * embeds its own Bun runtime, so we launch it directly instead of going
 * through Node + a separate bundle. Pass `binaryPath` so the entry uses
 * the absolute path found at install time (the user's PATH may differ).
 */
export function buildMaestroAgentMcpConfigEntry(binaryPath: string): AgentMcpEntry {
  return {
    command: binaryPath,
    args: ["mcp", "serve"],
  };
}

export function defaultAgentRuntimeTargets(home: string = homedir()): readonly AgentRuntimeTarget[] {
  return [
    { name: "Claude Code", configPath: join(home, ".claude", "mcp.json") },
    { name: "Codex", configPath: join(home, ".codex", "mcp.json") },
  ];
}

export function configureAgentRuntime(
  target: AgentRuntimeTarget,
  entry: AgentMcpEntry,
  options: { createIfMissing?: boolean } = {},
): ConfigureRuntimeResult {
  const createIfMissing = options.createIfMissing ?? true;
  const parentDir = dirname(target.configPath);
  const parentExists = existsSync(parentDir);

  if (!parentExists && !createIfMissing) {
    return { target, action: "skipped-no-runtime" };
  }

  let existing: Record<string, unknown> = {};
  if (existsSync(target.configPath)) {
    try {
      existing = JSON.parse(readFileSync(target.configPath, "utf8")) ?? {};
      if (typeof existing !== "object" || Array.isArray(existing)) existing = {};
    } catch (err) {
      return {
        target,
        action: "error",
        error: err instanceof Error ? err.message : String(err),
      };
    }
  }

  const mcpServers =
    typeof (existing as { mcpServers?: unknown }).mcpServers === "object" &&
    !Array.isArray((existing as { mcpServers?: unknown }).mcpServers)
      ? ((existing as { mcpServers: Record<string, unknown> }).mcpServers)
      : {};

  const previous = mcpServers[SERVER_KEY] as AgentMcpEntry | undefined;
  const wasIdentical =
    previous !== undefined &&
    previous.command === entry.command &&
    JSON.stringify(previous.args ?? []) === JSON.stringify(entry.args) &&
    JSON.stringify(previous.env ?? {}) === JSON.stringify(entry.env ?? {});

  if (wasIdentical) {
    return { target, action: "unchanged" };
  }

  const merged: AgentMcpEntry = previous
    ? { ...previous, command: entry.command, args: entry.args, env: previous.env ?? entry.env }
    : entry;

  const next = {
    ...existing,
    mcpServers: {
      ...mcpServers,
      [SERVER_KEY]: merged,
    },
  };

  try {
    if (!existsSync(parentDir)) {
      mkdirSync(parentDir, { recursive: true });
    }
    writeFileSync(target.configPath, `${JSON.stringify(next, null, 2)}\n`);
  } catch (err) {
    return {
      target,
      action: "error",
      error: err instanceof Error ? err.message : String(err),
    };
  }

  return { target, action: previous === undefined ? "created" : "updated" };
}
