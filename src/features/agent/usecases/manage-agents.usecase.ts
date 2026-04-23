import { createHash } from "node:crypto";
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
  writeJson,
  writeText,
  removeIfExists,
} from "@/shared/lib/fs.js";
import {
  removeReference,
  removeBlock,
  removeLegacyBlock,
} from "../lib/agent-block.js";
import { VERSION } from "@/shared/version.js";

export interface InjectResult {
  readonly agent: string;
  readonly action: "installed" | "migrated-to-skills" | "skipped" | "not-detected";
  readonly configPath: string;
  readonly installedSkills: readonly string[];
  readonly preservedUserEdits: readonly string[];
}

export interface RemoveResult {
  readonly agent: string;
  readonly action: "removed" | "not-found" | "not-detected";
  readonly configPath: string;
  readonly removedSkills: readonly string[];
}

type AgentConfigTargetScope = "all" | "home" | "project";

const BUNDLED_SKILL_PREFIX = "maestro-";
const MANIFEST_FILENAME = ".maestro-bundled.json";

/**
 * Marker written into each shipped skill directory. Its presence identifies
 * a dir as maestro-managed (so stale cleanup can delete it safely), and its
 * file-hash map lets us detect user edits between releases so we don't
 * silently clobber them.
 */
interface BundledSkillManifest {
  readonly managedBy: "maestro";
  readonly skillName: string;
  readonly installedAt: string;
  readonly maestroVersion: string;
  readonly fileHashes: Record<string, string>;
}

function contentHash(content: string): string {
  return createHash("sha256").update(content).digest("hex");
}

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

interface WriteBundledSkillResult {
  readonly changed: boolean;
  readonly preservedUserEdits: readonly string[];
}

interface BundledSkillCleanupResult {
  readonly changed: boolean;
  readonly preservedUserEdits: readonly string[];
}

async function readBundledSkillManifest(path: string): Promise<BundledSkillManifest | undefined> {
  const raw = await readText(path);
  if (raw === undefined) return undefined;
  try {
    return JSON.parse(raw) as BundledSkillManifest;
  } catch (error) {
    if (error instanceof SyntaxError) return undefined;
    throw error;
  }
}

/**
 * Write a single bundled skill into its install location. Uses a per-skill
 * manifest (`.maestro-bundled.json`) to distinguish user edits from stale
 * content so user edits are preserved across updates.
 *
 * Decision table per file (existing = on-disk content, shipped = new content
 * from the bundled template, prev = hash recorded in the prior manifest):
 *   - existing missing                    -> write, record new hash
 *   - existing == shipped                 -> no-op
 *   - existing != shipped, prev missing   -> migration; write, record new hash
 *   - existing != shipped, hash(existing) == prev -> no user edit; overwrite
 *   - existing != shipped, hash(existing) != prev -> user edit; preserve
 */
async function writeBundledSkill(
  skillRoot: string,
  template: BundledSkillTemplate,
): Promise<WriteBundledSkillResult> {
  const skillDir = join(skillRoot, template.name);
  const manifestPath = join(skillDir, MANIFEST_FILENAME);
  const prevManifest = await readBundledSkillManifest(manifestPath);

  const perFile = await Promise.all(template.files.map(async (file) => {
    const absolute = join(skillDir, file.path);
    const shippedHash = contentHash(file.content);
    const existing = await readText(absolute);

    if (existing === undefined) {
      await ensureDir(dirname(absolute));
      await writeText(absolute, file.content);
      return { path: file.path, manifestHash: shippedHash, changed: true, preserved: false };
    }

    if (existing === file.content) {
      return { path: file.path, manifestHash: shippedHash, changed: false, preserved: false };
    }

    const existingHash = contentHash(existing);
    const prevHash = prevManifest?.fileHashes?.[file.path];
    if (prevHash !== undefined && prevHash !== existingHash) {
      return { path: file.path, manifestHash: prevHash, changed: false, preserved: true };
    }

    if (prevHash === undefined) {
      // Migration path: no prior manifest entry for this file. Existing
      // content could be an older shipped version OR a user-authored file
      // that happens to share a path with something we now ship. We
      // overwrite (to complete the migration) but surface the action so
      // the user can notice if we stomped something they wanted kept.
      process.stderr.write(
        `[warn] migrated: overwriting pre-manifest file with shipped content: ${template.name}/${file.path}\n`,
      );
    }

    await writeText(absolute, file.content);
    return { path: file.path, manifestHash: shippedHash, changed: true, preserved: false };
  }));

  const staleCleanup = await removeStaleManagedFiles(
    skillDir,
    prevManifest,
    new Set(template.files.map((file) => file.path)),
  );

  const fileHashes: Record<string, string> = {};
  const preservedUserEdits = [...staleCleanup.preservedUserEdits.map((path) => `${template.name}/${path}`)];
  let changed = staleCleanup.changed;
  for (const result of perFile) {
    fileHashes[result.path] = result.manifestHash;
    if (result.changed) changed = true;
    if (result.preserved) {
      preservedUserEdits.push(`${template.name}/${result.path}`);
    }
  }

  await ensureDir(skillDir);
  const newManifest: BundledSkillManifest = {
    managedBy: "maestro",
    skillName: template.name,
    installedAt: new Date().toISOString(),
    maestroVersion: VERSION,
    fileHashes,
  };
  if (!prevManifest || !manifestsEqual(prevManifest, newManifest)) {
    await writeJson(manifestPath, newManifest);
  }

  return { changed, preservedUserEdits };
}

function manifestsEqual(a: BundledSkillManifest, b: BundledSkillManifest): boolean {
  if (a.skillName !== b.skillName) return false;
  const aKeys = Object.keys(a.fileHashes).sort();
  const bKeys = Object.keys(b.fileHashes).sort();
  if (aKeys.length !== bKeys.length) return false;
  for (let i = 0; i < aKeys.length; i++) {
    const key = aKeys[i]!;
    if (bKeys[i] !== key) return false;
    if (a.fileHashes[key] !== b.fileHashes[key]) return false;
  }
  return true;
}

async function removeStaleManagedFiles(
  skillDir: string,
  prevManifest: BundledSkillManifest | undefined,
  currentPaths: ReadonlySet<string>,
): Promise<BundledSkillCleanupResult> {
  if (!prevManifest) {
    return { changed: false, preservedUserEdits: [] };
  }

  const preservedUserEdits: string[] = [];
  let changed = false;
  for (const [relativePath, prevHash] of Object.entries(prevManifest.fileHashes)) {
    if (currentPaths.has(relativePath)) continue;

    const absolute = join(skillDir, relativePath);
    const existing = await readText(absolute);
    if (existing === undefined) continue;

    if (contentHash(existing) !== prevHash) {
      preservedUserEdits.push(relativePath);
      continue;
    }

    changed = await removeIfExists(absolute) || changed;
  }

  return { changed, preservedUserEdits };
}

/**
 * Remove any maestro-managed skill directory under the agent's skills root
 * that is no longer in the current bundled template set. A skill dir is
 * maestro-managed iff it contains the `.maestro-bundled.json` manifest.
 * User-authored directories sharing the `maestro-` prefix are left untouched.
 */
async function removeStaleBundledSkillDirs(skillRoot: string): Promise<string[]> {
  if (!(await dirExists(skillRoot))) return [];

  const shipped = new Set(BUNDLED_SKILL_TEMPLATES.map((template) => template.name));
  const entries = (await readdir(skillRoot, { withFileTypes: true }))
    .sort((left, right) => left.name.localeCompare(right.name));

  const results = await Promise.all(entries.map(async (entry) => {
    if (!entry.isDirectory()) return undefined;
    if (!entry.name.startsWith(BUNDLED_SKILL_PREFIX)) return undefined;
    if (shipped.has(entry.name)) return undefined;

    const manifestPath = join(skillRoot, entry.name, MANIFEST_FILENAME);
    const manifest = await readBundledSkillManifest(manifestPath);
    if (manifest?.managedBy !== "maestro") return undefined;

    await removeIfExists(join(skillRoot, entry.name), { recursive: true });
    return entry.name;
  }));

  return results.filter((name): name is string => name !== undefined);
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
      preservedUserEdits: [],
    };
  }

  await ensureDir(skillsRoot);
  const removedStaleSkillDirs = await removeStaleBundledSkillDirs(skillsRoot);

  const results = await Promise.all(
    BUNDLED_SKILL_TEMPLATES.map((template) => writeBundledSkill(skillsRoot, template)),
  );
  const anyChanged = removedStaleSkillDirs.length > 0 || results.some((r) => r.changed);
  const installed = BUNDLED_SKILL_TEMPLATES.map((template) => template.name);
  const preservedUserEdits = results.flatMap((r) => r.preservedUserEdits);

  for (const path of preservedUserEdits) {
    process.stderr.write(
      `[warn] preserved user-edited skill file: ~/${join(agent.configDir, "skills", path)} (not overwritten)\n`,
    );
  }

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
    preservedUserEdits,
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

  const [managedDirs, legacyRemoved] = await Promise.all([
    listManagedSkillDirs(skillsRoot),
    cleanupLegacyMaestroMd(agent, projectDir, homeDir),
  ]);

  const removed = await Promise.all(managedDirs.map(async (name) => {
    const skillDir = join(skillsRoot, name);
    return (await removeIfExists(skillDir, { recursive: true })) ? name : undefined;
  }));
  const removedNames = removed.filter((name): name is string => name !== undefined);
  const didSomething = removedNames.length > 0 || legacyRemoved;

  return {
    agent: agent.displayName,
    action: didSomething ? "removed" : "not-found",
    configPath: configDir,
    removedSkills: removedNames,
  };
}

/**
 * Enumerate every maestro-managed skill dir under `skillsRoot` (any dir
 * containing a `.maestro-bundled.json` manifest with `managedBy: "maestro"`),
 * regardless of whether the skill is still in the current bundled set.
 * `maestro uninstall` uses this to sweep skills that were dropped from the
 * bundle in a prior release so they don't orphan on disk.
 */
async function listManagedSkillDirs(skillsRoot: string): Promise<string[]> {
  if (!(await dirExists(skillsRoot))) return [];
  const entries = (await readdir(skillsRoot, { withFileTypes: true }))
    .sort((left, right) => left.name.localeCompare(right.name));
  const results = await Promise.all(entries.map(async (entry) => {
    if (!entry.isDirectory()) return undefined;
    if (!entry.name.startsWith(BUNDLED_SKILL_PREFIX)) return undefined;
    const manifest = await readBundledSkillManifest(
      join(skillsRoot, entry.name, MANIFEST_FILENAME),
    );
    return manifest?.managedBy === "maestro" ? entry.name : undefined;
  }));
  return results.filter((name): name is string => name !== undefined);
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
