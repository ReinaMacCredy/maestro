import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

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

export interface RunCliResult {
  readonly exitCode: number;
  readonly stderr: string;
}

export interface ConfigureRuntimeDeps {
  readonly which?: (cmd: string) => string | null;
  readonly runCli?: (argv: readonly string[]) => RunCliResult;
}

const SERVER_KEY = "maestro";

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

export function readMaestroEntry(target: AgentRuntimeTarget): AgentMcpEntry | undefined {
  switch (target.kind) {
    case "claude-code":
      return readClaudeUserScopeMaestro(target.configPath);
    case "codex":
      return readCodexMaestro(target.configPath);
  }
}

export function configureAgentRuntime(
  target: AgentRuntimeTarget,
  entry: AgentMcpEntry,
  deps: ConfigureRuntimeDeps = {},
): ConfigureRuntimeResult {
  const whichFn = deps.which ?? defaultWhich;
  const runCli = deps.runCli ?? defaultRunCli;

  if (!whichFn(target.cliBinary)) {
    return { target, action: "skipped-no-runtime" };
  }

  let existing: AgentMcpEntry | undefined;
  try {
    existing = readMaestroEntry(target);
  } catch (err) {
    return { target, action: "error", error: err instanceof Error ? err.message : String(err) };
  }

  if (existing && entriesEqual(existing, entry)) {
    return { target, action: "unchanged" };
  }

  if (existing) {
    const removeArgv = buildRemoveArgv(target);
    const removeResult = runCli(removeArgv);
    if (removeResult.exitCode !== 0) {
      return {
        target,
        action: "error",
        error: `${removeArgv.join(" ")} exited ${removeResult.exitCode}: ${removeResult.stderr.trim()}`,
      };
    }
  }

  const addArgv = buildAddArgv(target, entry);
  const addResult = runCli(addArgv);
  if (addResult.exitCode !== 0) {
    return {
      target,
      action: "error",
      error: `${addArgv.join(" ")} exited ${addResult.exitCode}: ${addResult.stderr.trim()}`,
    };
  }

  return { target, action: existing ? "updated" : "created" };
}

function buildAddArgv(target: AgentRuntimeTarget, entry: AgentMcpEntry): readonly string[] {
  switch (target.kind) {
    case "claude-code":
      return [target.cliBinary, "mcp", "add", SERVER_KEY, "-s", "user", "--", entry.command, ...entry.args];
    case "codex":
      return [target.cliBinary, "mcp", "add", SERVER_KEY, "--", entry.command, ...entry.args];
  }
}

function buildRemoveArgv(target: AgentRuntimeTarget): readonly string[] {
  switch (target.kind) {
    case "claude-code":
      return [target.cliBinary, "mcp", "remove", SERVER_KEY, "-s", "user"];
    case "codex":
      return [target.cliBinary, "mcp", "remove", SERVER_KEY];
  }
}

export function readClaudeUserScopeMaestro(configPath: string): AgentMcpEntry | undefined {
  if (!existsSync(configPath)) return undefined;
  const raw = readFileSync(configPath, "utf8");
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

export function readCodexMaestro(configPath: string): AgentMcpEntry | undefined {
  if (!existsSync(configPath)) return undefined;
  const raw = readFileSync(configPath, "utf8");
  const headerRe = /^\s*\[\s*mcp_servers\.\s*"?maestro"?\s*\]\s*$/;
  const lines = raw.split(/\r?\n/);
  let inSection = false;
  let command: string | undefined;
  let args: string[] | undefined;
  for (const line of lines) {
    const stripped = stripInlineComment(line);
    const trimmed = stripped.trim();
    if (trimmed.startsWith("[")) {
      if (inSection) break;
      if (headerRe.test(trimmed)) inSection = true;
      continue;
    }
    if (!inSection || trimmed.length === 0) continue;
    const cm = /^command\s*=\s*"((?:[^"\\]|\\.)*)"\s*$/.exec(trimmed);
    if (cm && cm[1] !== undefined) command = unescapeTomlBasicString(cm[1]);
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
  const re = /"((?:[^"\\]|\\.)*)"/g;
  for (const match of inner.matchAll(re)) out.push(unescapeTomlBasicString(match[1] ?? ""));
  return out;
}

function unescapeTomlBasicString(s: string): string {
  return s.replace(/\\(.)/g, (_, ch) => {
    switch (ch) {
      case "n":
        return "\n";
      case "t":
        return "\t";
      case "r":
        return "\r";
      case '"':
        return '"';
      case "\\":
        return "\\";
      default:
        return ch;
    }
  });
}

export function entriesEqual(a: AgentMcpEntry, b: AgentMcpEntry): boolean {
  if (a.command !== b.command) return false;
  const aArgs = a.args ?? [];
  const bArgs = b.args ?? [];
  if (aArgs.length !== bArgs.length) return false;
  for (let i = 0; i < aArgs.length; i++) {
    if (aArgs[i] !== bArgs[i]) return false;
  }
  return true;
}

function defaultWhich(cmd: string): string | null {
  return Bun.which(cmd);
}

function defaultRunCli(argv: readonly string[]): RunCliResult {
  const [cmd, ...rest] = argv;
  if (!cmd) return { exitCode: 1, stderr: "empty argv" };
  const proc = Bun.spawnSync({
    cmd: [cmd, ...rest],
    stdout: "pipe",
    stderr: "pipe",
  });
  return {
    exitCode: proc.exitCode ?? 1,
    stderr: proc.stderr.toString(),
  };
}
