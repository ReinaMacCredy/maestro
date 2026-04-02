import { homedir } from "node:os";
import { join } from "node:path";

export const BLOCK_START_MARKER = "<!-- maestro:start -->";
export const BLOCK_END_MARKER = "<!-- maestro:end -->";

export interface AgentConfigSpec {
  readonly slug: string;
  readonly displayName: string;
  readonly configDir: string;
  readonly configFile: string;
  readonly agentFlag: string;
}

export const SUPPORTED_AGENTS: readonly AgentConfigSpec[] = [
  { slug: "claude-code", displayName: "Claude Code", configDir: ".claude", configFile: "CLAUDE.md", agentFlag: "claude" },
  { slug: "codex", displayName: "Codex", configDir: ".codex", configFile: "AGENTS.md", agentFlag: "codex" },
  { slug: "droid", displayName: "Droid CLI", configDir: ".maestro", configFile: "AGENTS.md", agentFlag: "droid" },
  { slug: "gemini", displayName: "Gemini CLI", configDir: ".gemini", configFile: "GEMINI.md", agentFlag: "gemini" },
];

export function agentConfigPath(agent: AgentConfigSpec): string {
  return join(homedir(), agent.configDir, agent.configFile);
}

export function agentConfigDirPath(agent: AgentConfigSpec): string {
  return join(homedir(), agent.configDir);
}
