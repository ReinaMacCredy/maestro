import { homedir } from "node:os";
import { readdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import {
  SUPPORTED_AGENTS,
  agentConfigPath,
  agentConfigDirPath,
  agentReferencePath,
  agentLegacyConfigPaths,
  agentSkillsRoot,
  type AgentConfigSpec,
} from "../domain/agents.js";
import {
  BUNDLED_SKILL_TEMPLATES,
  type BundledSkillTemplate,
} from "@/infra/domain/bundled-skill-templates.js";
import {
  dirExists,
  ensureDir,
  readText,
  writeText,
  removeIfExists,
} from "@/shared/lib/fs.js";
import {
  removeReference,
  removeBlock,
  removeLegacyBlock,
} from "../lib/agent-block.js";

export interface InjectResult {
  readonly agent: string;
  readonly action: "installed" | "updated" | "migrated-to-skills" | "skipped" | "not-detected";
  readonly configPath: string;
  readonly installedSkills: readonly string[];
}

export interface RemoveResult {
  readonly agent: string;
  readonly action: "removed" | "not-found" | "not-detected";
  readonly configPath: string;
  readonly removedSkills: readonly string[];
}

type AgentConfigTargetScope = "all" | "home" | "project";

const BUNDLED_SKILL_PREFIX = "maestro-";

function agentMatchesTargetScope(
  agent: AgentConfigSpec,
  targetScope: AgentConfigTargetScope,
): boolean {
  if (targetScope === "all") return true;
  if (targetScope === "project") return agent.configScope === "project";
  return agent.configScope !== "project";
}

/**
 * Remove the legacy `~/.claude/MAESTRO.md` file and strip any `@MAESTRO.md`
 * reference line or old inline instruction blocks from the main config file.
 * Returns true if anything was removed (the caller uses this to distinguish
 * `migrated-to-skills` from `installed`).
 */
async function cleanupLegacyMaestroMd(
  agent: AgentConfigSpec,
  projectDir: string,
  homeDir: string,
): Promise<boolean> {
  let didSomething = false;

  const refPath = agentReferencePath(agent, projectDir, homeDir);
  if (await removeIfExists(refPath)) {
    didSomething = true;
  }

  const configPath = agentConfigPath(agent, projectDir, homeDir);
  const current = await readText(configPath);
  if (current === undefined) return didSomething;

  let cleaned = current;
  const refCleaned = removeReference(cleaned);
  if (refCleaned !== null) {
    cleaned = refCleaned;
    didSomething = true;
  }
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

  if (cleaned !== current) {
    await writeText(configPath, cleaned);
  }

  return didSomething;
}

/**
 * Write a single bundled skill into its install location. Returns true if
 * anything was actually written (content differed from what was on disk).
 */
async function writeBundledSkill(
  skillRoot: string,
  template: BundledSkillTemplate,
): Promise<boolean> {
  const skillDir = join(skillRoot, template.name);
  let changed = false;

  for (const file of template.files) {
    const absolute = join(skillDir, file.path);
    const existing = await readText(absolute);
    if (existing === file.content) continue;

    await ensureDir(dirname(absolute));
    await writeText(absolute, file.content);
    changed = true;
  }

  return changed;
}

/**
 * Remove any `maestro-*` skill directory under the agent's skills root that
 * does not correspond to a current bundled template. This keeps the install
 * clean when we rename or drop skills in future releases.
 */
async function removeStaleBundledSkillDirs(skillRoot: string): Promise<string[]> {
  if (!(await dirExists(skillRoot))) return [];

  const shipped = new Set(BUNDLED_SKILL_TEMPLATES.map((template) => template.name));
  const removed: string[] = [];

  const entries = (await readdir(skillRoot, { withFileTypes: true }))
    .sort((left, right) => left.name.localeCompare(right.name));

  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    if (!entry.name.startsWith(BUNDLED_SKILL_PREFIX)) continue;
    if (shipped.has(entry.name)) continue;

    const staleDir = join(skillRoot, entry.name);
    await removeIfExists(staleDir, { recursive: true });
    removed.push(entry.name);
  }

  return removed;
}

async function processInject(
  agent: AgentConfigSpec,
  projectDir: string,
  homeDir: string,
): Promise<InjectResult> {
  const configDir = agentConfigDirPath(agent, projectDir, homeDir);
  const skillsRoot = agentSkillsRoot(agent, projectDir, homeDir);

  if (!(await dirExists(configDir))) {
    return {
      agent: agent.displayName,
      action: "not-detected",
      configPath: configDir,
      installedSkills: [],
    };
  }

  await ensureDir(skillsRoot);
  await removeStaleBundledSkillDirs(skillsRoot);

  let anyChanged = false;
  const installed: string[] = [];
  for (const template of BUNDLED_SKILL_TEMPLATES) {
    const changed = await writeBundledSkill(skillsRoot, template);
    if (changed) anyChanged = true;
    installed.push(template.name);
  }

  const hadLegacy = await cleanupLegacyMaestroMd(agent, projectDir, homeDir);

  let action: InjectResult["action"];
  if (hadLegacy) {
    action = "migrated-to-skills";
  } else if (anyChanged) {
    action = "installed";
  } else {
    action = "skipped";
  }

  // If skills were already in sync but we still needed to cleanup nothing,
  // the action is "skipped". If skills were already in sync but we did
  // clean up legacy state, it's "migrated-to-skills". If some skill file
  // changed (new skill, edited SKILL.md, etc.), it's "installed" on first
  // run and effectively an update on subsequent runs. We keep a single
  // "installed" bucket for both to avoid action-sprawl; tests can look at
  // installedSkills to see what was written.

  return {
    agent: agent.displayName,
    action,
    configPath: configDir,
    installedSkills: installed,
  };
}

async function processRemove(
  agent: AgentConfigSpec,
  projectDir: string,
  homeDir: string,
): Promise<RemoveResult> {
  const configDir = agentConfigDirPath(agent, projectDir, homeDir);
  const skillsRoot = agentSkillsRoot(agent, projectDir, homeDir);

  if (!(await dirExists(configDir))) {
    return {
      agent: agent.displayName,
      action: "not-detected",
      configPath: configDir,
      removedSkills: [],
    };
  }

  let didSomething = false;
  const removed: string[] = [];

  for (const template of BUNDLED_SKILL_TEMPLATES) {
    const skillDir = join(skillsRoot, template.name);
    if (await removeIfExists(skillDir, { recursive: true })) {
      removed.push(template.name);
      didSomething = true;
    }
  }

  if (await cleanupLegacyMaestroMd(agent, projectDir, homeDir)) {
    didSomething = true;
  }

  return {
    agent: agent.displayName,
    action: didSomething ? "removed" : "not-found",
    configPath: configDir,
    removedSkills: removed,
  };
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

// Legacy helper paths — retained for callers that may still reference them
// (tests, older code paths). `agentLegacyConfigPaths` now always returns [].
export { agentLegacyConfigPaths };
