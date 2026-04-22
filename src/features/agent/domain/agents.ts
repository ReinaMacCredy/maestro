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

// droid and gemini will be re-added once skill support lands in those CLIs
export const SUPPORTED_AGENTS: readonly AgentConfigSpec[] = [
  { slug: "claude-code", displayName: "Claude Code", configDir: ".claude", configFile: "CLAUDE.md", agentFlag: "claude" },
  { slug: "codex", displayName: "Codex", configDir: ".codex", configFile: "AGENTS.md", agentFlag: "codex" },
];

/**
 * Directory where bundled maestro-* skills live for an agent, e.g.
 * `~/.claude/skills/` for Claude Code.
 */
export function agentSkillsRoot(agent: AgentConfigSpec, projectDir = process.cwd(), homeDir = homedir()): string {
  return agent.configScope === "project"
    ? join(projectDir, agent.configDir, "skills")
    : join(homeDir, agent.configDir, "skills");
}

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
  _agent: AgentConfigSpec,
  _projectDir = process.cwd(),
  _homeDir = homedir(),
): string[] {
  // The remaining supported agents (claude-code, codex) have no legacy config
  // paths to migrate from. Droid's .factory/.maestro fallbacks were dropped
  // when the droid integration was removed.
  return [];
}
