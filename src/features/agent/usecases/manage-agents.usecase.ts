import { homedir } from "node:os";
import {
  SUPPORTED_AGENTS,
  agentConfigPath,
  agentConfigDirPath,
  agentReferencePath,
  agentLegacyConfigPaths,
  type AgentConfigSpec,
} from "../domain/agents.js";
import { AGENT_INSTRUCTION_BLOCK } from "@/infra/domain/bootstrap-templates.js";
import { dirExists, ensureDir, readText, writeText, removeIfExists } from "@/shared/lib/fs.js";
import {
  hasReference,
  injectReference,
  removeReference,
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

type AgentConfigTargetScope = "all" | "home" | "project";

function agentMatchesTargetScope(
  agent: AgentConfigSpec,
  targetScope: AgentConfigTargetScope,
): boolean {
  if (targetScope === "all") return true;
  if (targetScope === "project") return agent.configScope === "project";
  return agent.configScope !== "project";
}

function stripLegacyAgentSections(content: string): {
  readonly cleaned: string;
  readonly hadLegacySections: boolean;
} {
  let cleaned = content;
  const blockCleaned = removeBlock(cleaned);
  if (blockCleaned !== null) cleaned = blockCleaned;
  const legacyCleaned = removeLegacyBlock(cleaned);
  if (legacyCleaned !== null) cleaned = legacyCleaned;

  return {
    cleaned,
    hadLegacySections: blockCleaned !== null || legacyCleaned !== null,
  };
}

async function processInject(agent: AgentConfigSpec, projectDir: string, homeDir: string): Promise<InjectResult> {
  const configPath = agentConfigPath(agent, projectDir, homeDir);
  const dirPath = agentConfigDirPath(agent, projectDir, homeDir);
  const refPath = agentReferencePath(agent, projectDir, homeDir);
  const targetContent = await readText(configPath);
  const legacySource = targetContent === undefined
    ? await firstExistingConfig(agentLegacyConfigPaths(agent, projectDir, homeDir))
    : undefined;

  if (!(await dirExists(dirPath)) && !legacySource) {
    return { agent: agent.displayName, action: "not-detected", configPath: refPath };
  }

  const rendered = renderBlock(agent);
  const existingRef = await readText(refPath);
  const migrating = targetContent === undefined && legacySource !== undefined;

  // Write MAESTRO.md -- skip if content already matches
  const refUpToDate = existingRef !== undefined && existingRef.trimEnd() === rendered.trimEnd();
  if (!refUpToDate) {
    await ensureDir(dirPath);
    await writeText(refPath, rendered + "\n");
  }

  // Ensure @MAESTRO.md reference exists in the main config file
  const mainContent = targetContent ?? legacySource?.content ?? "";
  const alreadyReferenced = hasReference(mainContent);
  const { cleaned, hadLegacySections } = stripLegacyAgentSections(mainContent);

  if (alreadyReferenced && refUpToDate && !hadLegacySections) {
    if (migrating) {
      // Content came from legacy path but reference already present -- write to target
      await ensureDir(dirPath);
      await writeText(configPath, mainContent);
      return { agent: agent.displayName, action: "migrated", configPath: refPath };
    }
    return { agent: agent.displayName, action: "skipped", configPath: refPath };
  }

  // Clean up old block markers and legacy headings before adding reference
  // Add the @MAESTRO.md reference
  const final = injectReference(cleaned);

  await ensureDir(dirPath);
  await writeText(configPath, final);

  const action = migrating || hadLegacySections ? "migrated"
    : alreadyReferenced ? "updated"
    : "injected";

  return { agent: agent.displayName, action, configPath: refPath };
}

async function processRemove(agent: AgentConfigSpec, projectDir: string, homeDir: string): Promise<RemoveResult> {
  const configPath = agentConfigPath(agent, projectDir, homeDir);
  const refPath = agentReferencePath(agent, projectDir, homeDir);
  const current = await firstExistingConfig([
    configPath,
    ...agentLegacyConfigPaths(agent, projectDir, homeDir),
  ]);

  if (!current) {
    return { agent: agent.displayName, action: "not-detected", configPath: refPath };
  }

  let didSomething = false;

  // Remove MAESTRO.md file
  const fileRemoved = await removeIfExists(refPath);
  if (fileRemoved) didSomething = true;

  // Remove @MAESTRO.md reference from main config
  let cleaned = removeReference(current.content);
  if (cleaned !== null) {
    didSomething = true;
  } else {
    cleaned = current.content;
  }

  // Also clean up any remaining old block markers or legacy headings
  const blockCleaned = removeBlock(cleaned);
  if (blockCleaned !== null) {
    cleaned = blockCleaned;
    didSomething = true;
  }
  const legacyCleaned = removeLegacyBlock(cleaned);
  if (legacyCleaned !== null) {
    cleaned = legacyCleaned;
    didSomething = true;
  }

  if (!didSomething) {
    return { agent: agent.displayName, action: "not-found", configPath: refPath };
  }

  if (cleaned !== current.content) {
    await writeText(current.path, cleaned);
  }

  return { agent: agent.displayName, action: "removed", configPath: refPath };
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

export async function injectAgentBlocks(
  projectDir = process.cwd(),
  targetScope: AgentConfigTargetScope = "all",
  homeDir?: string,
): Promise<InjectResult[]> {
  const resolvedHomeDir = homeDir ?? homedir();
  return Promise.all(
    SUPPORTED_AGENTS
      .filter((agent) => agentMatchesTargetScope(agent, targetScope))
      .map((agent) => processInject(agent, projectDir, resolvedHomeDir)),
  );
}

export async function removeAgentBlocks(
  projectDir = process.cwd(),
  targetScope: AgentConfigTargetScope = "all",
  homeDir?: string,
): Promise<RemoveResult[]> {
  const resolvedHomeDir = homeDir ?? homedir();
  return Promise.all(
    SUPPORTED_AGENTS
      .filter((agent) => agentMatchesTargetScope(agent, targetScope))
      .map((agent) => processRemove(agent, projectDir, resolvedHomeDir)),
  );
}
