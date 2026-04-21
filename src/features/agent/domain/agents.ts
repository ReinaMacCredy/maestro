import { homedir } from "node:os";
import { join } from "node:path";

export const BLOCK_START_MARKER = "<!-- maestro:start -->";
export const BLOCK_END_MARKER = "<!-- maestro:end -->";

export const REFERENCE_FILE = "MAESTRO.md";

export interface AgentConfigSpec {
  readonly slug: string;
  readonly displayName: string;
  readonly configDir: string;
  readonly configFile: string;
  readonly agentFlag: string;
  readonly configScope?: "home" | "project";
}

export const SUPPORTED_AGENTS: readonly AgentConfigSpec[] = [
  { slug: "claude-code", displayName: "Claude Code", configDir: ".claude", configFile: "CLAUDE.md", agentFlag: "claude" },
  { slug: "codex", displayName: "Codex", configDir: ".codex", configFile: "AGENTS.md", agentFlag: "codex" },
  { slug: "droid", displayName: "Droid CLI", configDir: ".maestro", configFile: "AGENTS.md", agentFlag: "droid", configScope: "project" },
  { slug: "gemini", displayName: "Gemini CLI", configDir: ".gemini", configFile: "GEMINI.md", agentFlag: "gemini" },
];

export function agentConfigPath(agent: AgentConfigSpec, projectDir = process.cwd(), homeDir = homedir()): string {
  return agent.configScope === "project"
    ? join(projectDir, agent.configDir, agent.configFile)
    : join(homeDir, agent.configDir, agent.configFile);
}

export function agentConfigDirPath(agent: AgentConfigSpec, projectDir = process.cwd(), homeDir = homedir()): string {
  return agent.configScope === "project"
    ? join(projectDir, agent.configDir)
    : join(homeDir, agent.configDir);
}

export function agentReferencePath(agent: AgentConfigSpec, projectDir = process.cwd(), homeDir = homedir()): string {
  return agent.configScope === "project"
    ? join(projectDir, agent.configDir, REFERENCE_FILE)
    : join(homeDir, agent.configDir, REFERENCE_FILE);
}

export function agentLegacyConfigPaths(
  agent: AgentConfigSpec,
  projectDir = process.cwd(),
  homeDir = homedir(),
): string[] {
  if (agent.slug !== "droid") {
    return [];
  }

  return [
    join(projectDir, ".factory", "AGENTS.md"),
    join(homeDir, ".factory", "AGENTS.md"),
    join(homeDir, ".maestro", "AGENTS.md"),
  ].filter((path) => path !== agentConfigPath(agent, projectDir, homeDir));
}
