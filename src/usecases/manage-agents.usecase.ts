import {
  SUPPORTED_AGENTS,
  agentConfigPath,
  agentConfigDirPath,
  type AgentConfigSpec,
} from "../domain/agents.js";
import { AGENT_INSTRUCTION_BLOCK } from "../domain/defaults.js";
import { renderTemplate } from "../lib/template.js";
import { dirExists, readText, writeText } from "../lib/fs.js";
import {
  extractBlock,
  injectBlock,
  replaceBlock,
  removeBlock,
  removeLegacyBlock,
} from "../lib/agent-block.js";

export interface InjectResult {
  readonly agent: string;
  readonly action: "injected" | "updated" | "migrated" | "skipped" | "not-detected";
  readonly configPath: string;
}

export interface RemoveResult {
  readonly agent: string;
  readonly action: "removed" | "not-found" | "not-detected";
  readonly configPath: string;
}

function renderBlock(agent: AgentConfigSpec): string {
  return renderTemplate(AGENT_INSTRUCTION_BLOCK, { agent: agent.agentFlag });
}

async function processInject(agent: AgentConfigSpec): Promise<InjectResult> {
  const configPath = agentConfigPath(agent);
  const dirPath = agentConfigDirPath(agent);

  if (!(await dirExists(dirPath))) {
    return { agent: agent.displayName, action: "not-detected", configPath };
  }

  const rendered = renderBlock(agent);
  const existing = await readText(configPath) ?? "";
  const currentBlock = extractBlock(existing);

  if (currentBlock === null) {
    const cleaned = removeLegacyBlock(existing);
    if (cleaned !== null) {
      await writeText(configPath, injectBlock(cleaned, rendered));
      return { agent: agent.displayName, action: "migrated", configPath };
    }

    await writeText(configPath, injectBlock(existing, rendered));
    return { agent: agent.displayName, action: "injected", configPath };
  }

  if (currentBlock === rendered) {
    return { agent: agent.displayName, action: "skipped", configPath };
  }

  await writeText(configPath, replaceBlock(existing, rendered)!);
  return { agent: agent.displayName, action: "updated", configPath };
}

async function processRemove(agent: AgentConfigSpec): Promise<RemoveResult> {
  const configPath = agentConfigPath(agent);
  const dirPath = agentConfigDirPath(agent);

  if (!(await dirExists(dirPath))) {
    return { agent: agent.displayName, action: "not-detected", configPath };
  }

  const existing = await readText(configPath);
  if (!existing) {
    return { agent: agent.displayName, action: "not-found", configPath };
  }

  const cleaned = removeBlock(existing);
  if (!cleaned) {
    return { agent: agent.displayName, action: "not-found", configPath };
  }

  await writeText(configPath, cleaned);
  return { agent: agent.displayName, action: "removed", configPath };
}

export async function injectAgentBlocks(): Promise<InjectResult[]> {
  return Promise.all(SUPPORTED_AGENTS.map(processInject));
}

export async function removeAgentBlocks(): Promise<RemoveResult[]> {
  return Promise.all(SUPPORTED_AGENTS.map(processRemove));
}
