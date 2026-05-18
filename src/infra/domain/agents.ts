import { homedir } from "node:os";
import { join } from "node:path";
import {
  resolveAgentSkillsSharedRoot,
  resolveCodexHome,
  resolveHermesConfigPath,
  resolveHermesHome,
  resolveHermesSkillsRoot,
} from "@/shared/domain/defaults.js";

export const BLOCK_START_MARKER = "<!-- maestro:start -->";
export const BLOCK_END_MARKER = "<!-- maestro:end -->";

export const REFERENCE_FILE = "MAESTRO.md";

// Marker pair and reference file used by `maestro setup` when seeding the
// project-root `AGENTS.md` and `CLAUDE.md`. Distinct from the legacy values
// above so `cleanupLegacyMaestroMd` in `manage-agents.usecase.ts` continues
// to strip pre-redesign installs without touching the new seed.
export const SETUP_BLOCK_START_MARKER = "<!-- maestro-setup:start -->";
export const SETUP_BLOCK_END_MARKER = "<!-- maestro-setup:end -->";

export const SETUP_REFERENCE_FILE = "AGENTS.md";

export interface AgentConfigSpec {
  readonly slug: string;
  readonly displayName: string;
  readonly providerId: "codex" | "claude" | "hermes" | "agentskills";
  readonly configDir?: string;
  readonly configFile?: string;
  readonly agentFlag: string;
  readonly configScope?: "home" | "project";
  readonly runtime: boolean;
  readonly skillTarget: boolean;
  readonly alwaysDetected?: boolean;
  readonly binary?: string;
}

export const SUPPORTED_AGENTS: readonly AgentConfigSpec[] = [
  {
    slug: "claude-code",
    providerId: "claude",
    displayName: "Claude Code",
    configDir: ".claude",
    configFile: "CLAUDE.md",
    agentFlag: "claude",
    runtime: true,
    skillTarget: true,
    binary: "claude",
  },
  {
    slug: "codex",
    providerId: "codex",
    displayName: "Codex",
    configDir: ".codex",
    configFile: "AGENTS.md",
    agentFlag: "codex",
    runtime: true,
    skillTarget: true,
    binary: "codex",
  },
  {
    slug: "hermes",
    providerId: "hermes",
    displayName: "Hermes",
    configDir: ".hermes",
    configFile: "config.yaml",
    agentFlag: "hermes",
    runtime: true,
    skillTarget: true,
    alwaysDetected: true,
    binary: "hermes",
  },
  {
    slug: "agentskills",
    providerId: "agentskills",
    displayName: "AgentSkills",
    agentFlag: "agentskills",
    runtime: false,
    skillTarget: true,
    alwaysDetected: true,
  },
];

export const RUNTIME_AGENTS = SUPPORTED_AGENTS.filter((agent) => agent.runtime);
export const SKILL_TARGET_AGENTS = SUPPORTED_AGENTS.filter((agent) => agent.skillTarget);

/**
 * Resolve the agent's home-scoped config root. For Codex this honors the
 * `CODEX_HOME` env var; other agents resolve to `<home>/<configDir>`.
 */
function resolveAgentHomeRoot(agent: AgentConfigSpec, homeDir: string): string {
  if (agent.slug === "codex") return resolveCodexHome(homeDir);
  if (agent.slug === "hermes") return resolveHermesHome(homeDir);
  if (agent.slug === "agentskills") return resolveAgentSkillsSharedRoot(homeDir);
  if (agent.configDir === undefined) return homeDir;
  return join(homeDir, agent.configDir);
}

/**
 * Directory where bundled maestro-* skills live for an agent, e.g.
 * `~/.claude/skills/` for Claude Code.
 */
export function agentSkillsRoot(agent: AgentConfigSpec, projectDir = process.cwd(), homeDir = homedir()): string {
  if (agent.slug === "agentskills") return resolveAgentSkillsSharedRoot(homeDir);
  if (agent.slug === "hermes") return resolveHermesSkillsRoot(homeDir);
  if (agent.configScope === "project") {
    return join(projectDir, agent.configDir ?? "", "skills");
  }
  return join(resolveAgentHomeRoot(agent, homeDir), "skills");
}

export function agentConfigPath(agent: AgentConfigSpec, projectDir = process.cwd(), homeDir = homedir()): string {
  if (agent.slug === "hermes") return resolveHermesConfigPath(homeDir);
  if (agent.configFile === undefined) return agentConfigDirPath(agent, projectDir, homeDir);
  if (agent.configScope === "project") {
    return join(projectDir, agent.configDir ?? "", agent.configFile);
  }
  return join(resolveAgentHomeRoot(agent, homeDir), agent.configFile);
}

export function agentConfigDirPath(agent: AgentConfigSpec, projectDir = process.cwd(), homeDir = homedir()): string {
  if (agent.configScope === "project") {
    return join(projectDir, agent.configDir ?? "");
  }
  return resolveAgentHomeRoot(agent, homeDir);
}

export function agentReferencePath(agent: AgentConfigSpec, projectDir = process.cwd(), homeDir = homedir()): string {
  if (agent.configFile === undefined) return agentConfigDirPath(agent, projectDir, homeDir);
  if (agent.configScope === "project") {
    return join(projectDir, agent.configDir ?? "", REFERENCE_FILE);
  }
  return join(resolveAgentHomeRoot(agent, homeDir), REFERENCE_FILE);
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
