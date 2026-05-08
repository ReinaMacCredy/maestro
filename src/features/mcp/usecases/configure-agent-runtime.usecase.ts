import { homedir } from "node:os";
import { join } from "node:path";
import { readText } from "@/shared/lib/fs.js";
import { execArgv, type ShellResult } from "@/shared/lib/shell.js";

export interface AgentMcpEntry {
  readonly command: string;
  readonly args: readonly string[];
}

export type RuntimeKind = "claude-code" | "codex";

export interface AgentRuntimeTarget {
  readonly name: string;
  readonly kind: RuntimeKind;
  readonly cliBinary: string;
  readonly configPath: string;
}

export interface ConfigureRuntimeResult {
  readonly target: AgentRuntimeTarget;
  readonly action: "skipped-no-runtime" | "created" | "updated" | "unchanged" | "error";
  readonly error?: string;
}

export interface ConfigureRuntimeDeps {
  readonly which?: (cmd: string) => string | null;
  readonly runCli?: (argv: string[]) => Promise<ShellResult>;
}

const SERVER_KEY = "maestro";

interface RuntimeKindConfig {
  readonly addExtraFlags: readonly string[];
  readonly removeExtraFlags: readonly string[];
  readonly readEntry: (configPath: string) => Promise<AgentMcpEntry | undefined>;
}

const RUNTIME_KIND_CONFIG: Record<RuntimeKind, RuntimeKindConfig> = {
  "claude-code": {
    addExtraFlags: ["-s", "user"],
    removeExtraFlags: ["-s", "user"],
    readEntry: readClaudeUserScopeMaestro,
  },
  codex: {
    addExtraFlags: [],
    removeExtraFlags: [],
    readEntry: readCodexMaestro,
  },
};

export function resolveMaestroBinaryInstallPath(
  installDir: string,
  platform: NodeJS.Platform = process.platform,
): string {
  return join(installDir, platform === "win32" ? "maestro.exe" : "maestro");
}

export function buildMaestroAgentMcpConfigEntry(binaryPath: string): AgentMcpEntry {
  return {
    command: binaryPath,
    args: ["mcp", "serve"],
  };
}

export function defaultAgentRuntimeTargets(home: string = homedir()): readonly AgentRuntimeTarget[] {
  return [
    {
      name: "Claude Code",
      kind: "claude-code",
      cliBinary: "claude",
      configPath: join(home, ".claude.json"),
    },
    {
      name: "Codex",
      kind: "codex",
      cliBinary: "codex",
      configPath: join(home, ".codex", "config.toml"),
    },
  ];
}

export async function readMaestroEntry(target: AgentRuntimeTarget): Promise<AgentMcpEntry | undefined> {
  return RUNTIME_KIND_CONFIG[target.kind].readEntry(target.configPath);
}

export async function configureAgentRuntime(
  target: AgentRuntimeTarget,
  entry: AgentMcpEntry,
  deps: ConfigureRuntimeDeps = {},
): Promise<ConfigureRuntimeResult> {
  const whichFn = deps.which ?? defaultWhich;
  const runCli = deps.runCli ?? execArgv;
  const kindConfig = RUNTIME_KIND_CONFIG[target.kind];

  if (!whichFn(target.cliBinary)) {
    return { target, action: "skipped-no-runtime" };
  }

  let existing: AgentMcpEntry | undefined;
  try {
    existing = await kindConfig.readEntry(target.configPath);
  } catch (err) {
    return { target, action: "error", error: err instanceof Error ? err.message : String(err) };
  }

  if (existing && entriesEqual(existing, entry)) {
    return { target, action: "unchanged" };
  }

  if (existing) {
    const removeArgv = [target.cliBinary, "mcp", "remove", SERVER_KEY, ...kindConfig.removeExtraFlags];
    const removeResult = await runCli(removeArgv);
    if (removeResult.exitCode !== 0) {
      return {
        target,
        action: "error",
        error: `${removeArgv.join(" ")} exited ${removeResult.exitCode}: ${removeResult.stderr.trim()}`,
      };
    }
  }

  const addArgv = [
    target.cliBinary,
    "mcp",
    "add",
    SERVER_KEY,
    ...kindConfig.addExtraFlags,
    "--",
    entry.command,
    ...entry.args,
  ];
  const addResult = await runCli(addArgv);
  if (addResult.exitCode !== 0) {
    return {
      target,
      action: "error",
      error: `${addArgv.join(" ")} exited ${addResult.exitCode}: ${addResult.stderr.trim()}`,
    };
  }

  return { target, action: existing ? "updated" : "created" };
}

export async function readClaudeUserScopeMaestro(configPath: string): Promise<AgentMcpEntry | undefined> {
  const raw = await readText(configPath);
  if (raw === undefined) return undefined;
  const json = JSON.parse(raw);
  const servers = json?.mcpServers;
  if (!servers || typeof servers !== "object") return undefined;
  const entry = (servers as Record<string, unknown>)[SERVER_KEY];
  if (!entry || typeof entry !== "object") return undefined;
  const command = (entry as { command?: unknown }).command;
  if (typeof command !== "string") return undefined;
  const argsRaw = (entry as { args?: unknown }).args;
  const args = Array.isArray(argsRaw) ? argsRaw.filter((v): v is string => typeof v === "string") : [];
  return { command, args };
}

export async function readCodexMaestro(configPath: string): Promise<AgentMcpEntry | undefined> {
  const raw = await readText(configPath);
  if (raw === undefined) return undefined;
  const headerRe = /^\s*\[\s*mcp_servers\.\s*"?maestro"?\s*\]\s*$/;
  const lines = raw.split(/\r?\n/);
  let inSection = false;
  let command: string | undefined;
  let args: string[] | undefined;
  for (const line of lines) {
    const trimmed = stripInlineComment(line).trim();
    if (trimmed.startsWith("[")) {
      if (inSection) break;
      if (headerRe.test(trimmed)) inSection = true;
      continue;
    }
    if (!inSection || trimmed.length === 0) continue;
    const cm = /^command\s*=\s*"((?:[^"\\]|\\.)*)"\s*$/.exec(trimmed);
    if (cm && cm[1] !== undefined) command = unescapeTomlPath(cm[1]);
    const am = /^args\s*=\s*\[(.*)\]\s*$/.exec(trimmed);
    if (am && am[1] !== undefined) args = parseTomlStringArray(am[1]);
  }
  if (!command) return undefined;
  return { command, args: args ?? [] };
}

function stripInlineComment(line: string): string {
  let inString = false;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (ch === "\\" && inString) {
      i++;
      continue;
    }
    if (ch === '"') inString = !inString;
    else if (ch === "#" && !inString) return line.slice(0, i);
  }
  return line;
}

function parseTomlStringArray(inner: string): string[] {
  const out: string[] = [];
  for (const match of inner.matchAll(/"((?:[^"\\]|\\.)*)"/g)) out.push(unescapeTomlPath(match[1] ?? ""));
  return out;
}

// We only ever read paths and CLI argv, so the only escape sequences that
// can appear in practice are \" and \\. Other TOML escapes (\n, \t, etc.)
// are not handled; if they ever appeared we would surface them literally.
function unescapeTomlPath(s: string): string {
  return s.replace(/\\(["\\])/g, "$1");
}

export function entriesEqual(a: AgentMcpEntry, b: AgentMcpEntry): boolean {
  if (a.command !== b.command) return false;
  if (a.args.length !== b.args.length) return false;
  for (let i = 0; i < a.args.length; i++) {
    if (a.args[i] !== b.args[i]) return false;
  }
  return true;
}

function defaultWhich(cmd: string): string | null {
  return Bun.which(cmd);
}
