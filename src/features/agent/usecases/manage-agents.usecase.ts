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
  const results = await Promise.all(template.files.map(async (file) => {
    const absolute = join(skillDir, file.path);
    const existing = await readText(absolute);
    if (existing === file.content) return false;

    await ensureDir(dirname(absolute));
    await writeText(absolute, file.content);
    return true;
  }));
  return results.some(Boolean);
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

  const changes = await Promise.all(
    BUNDLED_SKILL_TEMPLATES.map((template) => writeBundledSkill(skillsRoot, template)),
  );
  const anyChanged = changes.some(Boolean);
  const installed = BUNDLED_SKILL_TEMPLATES.map((template) => template.name);

  const hadLegacy = await cleanupLegacyMaestroMd(agent, projectDir, homeDir);

  const action: InjectResult["action"] = hadLegacy
    ? "migrated-to-skills"
    : anyChanged
      ? "installed"
      : "skipped";

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

  const [skillResults, legacyRemoved] = await Promise.all([
    Promise.all(
      BUNDLED_SKILL_TEMPLATES.map(async (template) => {
        const skillDir = join(skillsRoot, template.name);
        return (await removeIfExists(skillDir, { recursive: true }))
          ? template.name
          : undefined;
      }),
    ),
    cleanupLegacyMaestroMd(agent, projectDir, homeDir),
  ]);

  const removed = skillResults.filter((name): name is string => name !== undefined);
  const didSomething = removed.length > 0 || legacyRemoved;

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
