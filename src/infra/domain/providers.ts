import { homedir } from "node:os";
import {
  agentConfigDirPath,
  agentConfigPath,
  agentSkillsRoot,
  SUPPORTED_AGENTS,
  type AgentConfigSpec,
} from "./agents.js";

export type ProviderId = "codex" | "claude" | "hermes" | "agentskills";

export interface ProviderDescriptor {
  readonly id: ProviderId;
  readonly slug: string;
  readonly displayName: string;
  readonly runtime: boolean;
  readonly skillTarget: boolean;
  readonly binary?: string;
  readonly configPath: string;
  readonly configDir: string;
  readonly skillsRoot: string;
}

export function listProviders(projectDir = process.cwd(), homeDir = homedir()): readonly ProviderDescriptor[] {
  return SUPPORTED_AGENTS.map((agent) => describeProvider(agent, projectDir, homeDir));
}

export function getProvider(
  idOrSlug: string,
  projectDir = process.cwd(),
  homeDir = homedir(),
): ProviderDescriptor | undefined {
  return listProviders(projectDir, homeDir).find((provider) =>
    provider.id === idOrSlug || provider.slug === idOrSlug
  );
}

export function listSkillTargetProviders(
  projectDir = process.cwd(),
  homeDir = homedir(),
): readonly ProviderDescriptor[] {
  return SUPPORTED_AGENTS
    .filter((agent) => agent.skillTarget)
    .map((agent) => describeProvider(agent, projectDir, homeDir));
}

function describeProvider(
  agent: AgentConfigSpec,
  projectDir: string,
  homeDir: string,
): ProviderDescriptor {
  return {
    id: agent.providerId,
    slug: agent.slug,
    displayName: agent.displayName,
    runtime: agent.runtime,
    skillTarget: agent.skillTarget,
    ...(agent.binary ? { binary: agent.binary } : {}),
    configPath: agentConfigPath(agent, projectDir, homeDir),
    configDir: agentConfigDirPath(agent, projectDir, homeDir),
    skillsRoot: agentSkillsRoot(agent, projectDir, homeDir),
  };
}
