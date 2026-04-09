import {
  SUPPORTED_AGENTS,
  agentConfigPath,
  agentConfigDirPath,
  agentLegacyConfigPaths,
  type AgentConfigSpec,
} from "../domain/agents.js";
import { AGENT_INSTRUCTION_BLOCK } from "../domain/defaults.js";
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

/**
 * Phase 1 strip: the instruction block no longer contains an `{{agent}}`
 * placeholder, so rendering collapsed to a static constant. The
 * per-agent parameter is retained for readability at call sites.
 */
function renderBlock(_agent: AgentConfigSpec): string {
  return AGENT_INSTRUCTION_BLOCK;
}

interface ExistingConfig {
  readonly path: string;
  readonly content: string;
}

async function processInject(agent: AgentConfigSpec, projectDir: string): Promise<InjectResult> {
  const configPath = agentConfigPath(agent, projectDir);
  const dirPath = agentConfigDirPath(agent, projectDir);
  const targetContent = await readText(configPath);
  const legacySource = targetContent === undefined
    ? await firstExistingConfig(agentLegacyConfigPaths(agent, projectDir))
    : undefined;

  if (!(await dirExists(dirPath)) && !legacySource) {
    return { agent: agent.displayName, action: "not-detected", configPath };
  }

  const rendered = renderBlock(agent);
  const existing = targetContent ?? legacySource?.content ?? "";
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
  const current = await firstExistingConfig([
    configPath,
    ...agentLegacyConfigPaths(agent, projectDir),
  ]);

  if (!current) {
    return { agent: agent.displayName, action: "not-detected", configPath };
  }

  const cleaned = removeBlock(current.content) ?? removeLegacyBlock(current.content);
  if (!cleaned) {
    return { agent: agent.displayName, action: "not-found", configPath: current.path };
  }

  await writeText(current.path, cleaned);
  return { agent: agent.displayName, action: "removed", configPath: current.path };
}

async function firstExistingConfig(paths: readonly string[]): Promise<ExistingConfig | undefined> {
  for (const path of paths) {
    const content = await readText(path);
    if (content !== undefined) {
      return { path, content };
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
