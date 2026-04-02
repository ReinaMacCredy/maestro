import {
  SUPPORTED_AGENTS,
  agentConfigPath,
  agentConfigDirPath,
  agentLegacyConfigPaths,
  type AgentConfigSpec,
} from "../domain/agents.js";
import { AGENT_INSTRUCTION_BLOCK } from "../domain/defaults.js";
import { renderTemplate } from "../lib/template.js";
import { dirExists, ensureDir, readText, writeText } from "../lib/fs.js";
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

async function processInject(agent: AgentConfigSpec, projectDir: string): Promise<InjectResult> {
  const configPath = agentConfigPath(agent, projectDir);
  const dirPath = agentConfigDirPath(agent, projectDir);
  const dirPresent = await dirExists(dirPath);
  const legacySource = await firstExistingPath(agentLegacyConfigPaths(agent, projectDir));
  const targetContent = await readText(configPath);

  if (!dirPresent && !legacySource) {
    return { agent: agent.displayName, action: "not-detected", configPath };
  }

  const rendered = renderBlock(agent);
  const existing = targetContent ?? (legacySource ? (await readText(legacySource)) ?? "" : "");
  const currentBlock = extractBlock(existing);
  const migrating = targetContent === undefined && legacySource !== undefined;

  if (currentBlock === null) {
    const cleaned = removeLegacyBlock(existing);
    if (cleaned !== null) {
      await ensureDir(dirPath);
      await writeText(configPath, injectBlock(cleaned, rendered));
      return { agent: agent.displayName, action: "migrated", configPath };
    }

    await ensureDir(dirPath);
    await writeText(configPath, injectBlock(existing, rendered));
    return { agent: agent.displayName, action: migrating ? "migrated" : "injected", configPath };
  }

  if (currentBlock === rendered) {
    if (migrating) {
      await ensureDir(dirPath);
      await writeText(configPath, existing);
      return { agent: agent.displayName, action: "migrated", configPath };
    }
    return { agent: agent.displayName, action: "skipped", configPath };
  }

  await ensureDir(dirPath);
  await writeText(configPath, replaceBlock(existing, rendered)!);
  return { agent: agent.displayName, action: migrating ? "migrated" : "updated", configPath };
}

async function processRemove(agent: AgentConfigSpec, projectDir: string): Promise<RemoveResult> {
  const configPath = agentConfigPath(agent, projectDir);
  const dirPath = agentConfigDirPath(agent, projectDir);
  const legacyPath = await firstExistingPath(agentLegacyConfigPaths(agent, projectDir));
  const actualPath = await dirExists(dirPath) ? configPath : legacyPath;

  if (!actualPath) {
    return { agent: agent.displayName, action: "not-detected", configPath };
  }

  const existing = await readText(actualPath);
  if (!existing) {
    return { agent: agent.displayName, action: "not-found", configPath: actualPath };
  }

  const cleaned = removeBlock(existing) ?? removeLegacyBlock(existing);
  if (!cleaned) {
    return { agent: agent.displayName, action: "not-found", configPath: actualPath };
  }

  await writeText(actualPath, cleaned);
  return { agent: agent.displayName, action: "removed", configPath: actualPath };
}

async function firstExistingPath(paths: readonly string[]): Promise<string | undefined> {
  for (const path of paths) {
    if ((await readText(path)) !== undefined) {
      return path;
    }
  }

  return undefined;
}

export async function injectAgentBlocks(projectDir = process.cwd()): Promise<InjectResult[]> {
  return Promise.all(SUPPORTED_AGENTS.map((agent) => processInject(agent, projectDir)));
}

export async function removeAgentBlocks(projectDir = process.cwd()): Promise<RemoveResult[]> {
  return Promise.all(SUPPORTED_AGENTS.map((agent) => processRemove(agent, projectDir)));
}
