import { createHash } from "node:crypto";
import { homedir } from "node:os";
import { chmod, copyFile, lstat, readdir, realpath } from "node:fs/promises";
import type { Dirent } from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import {
  SKILL_TARGET_AGENTS,
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
  readlinkSafe,
  readText,
  symlinkDir,
  writeJson,
  writeText,
  removeIfExists,
} from "@/shared/lib/fs.js";
import { resolveMaestroSkillsRoot } from "@/shared/domain/defaults.js";
import {
  removeReference,
  removeBlock,
  removeLegacyBlock,
} from "../lib/agent-block.js";
import { VERSION } from "@/shared/version.js";
import { parseYaml, stringifyYaml } from "@/shared/lib/yaml.js";
import { resolveAgentSkillsSharedRoot } from "@/shared/domain/defaults.js";

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

const BUNDLED_SKILL_NAMES: readonly string[] = BUNDLED_SKILL_TEMPLATES.map((t) => t.name);
const BUNDLED_SKILL_NAME_SET: ReadonlySet<string> = new Set(BUNDLED_SKILL_NAMES);

/**
 * Whether a symlink target lives under the maestro skills tree. Uses a
 * separator-aware prefix check so `~/.maestro/skills-extra/foo` doesn't get
 * misclassified as living under `~/.maestro/skills/`.
 */
function isMaestroTreeLink(target: string, maestroSkillsRoot: string): boolean {
  return target === maestroSkillsRoot || target.startsWith(maestroSkillsRoot + sep);
}

function formatPathUnderRoot(root: string, relativePath: string): string {
  if (relativePath.length === 0) return root;
  return root.endsWith(sep) ? `${root}${relativePath}` : `${root}${sep}${relativePath}`;
}

/**
 * Marker written into each shipped skill directory under the maestro source
 * of truth (`~/.maestro/skills/<skill>/`). Its presence identifies a dir as
 * maestro-managed (so stale cleanup can delete it safely), and its file-hash
 * map lets us detect user edits between releases so we don't silently
 * clobber them.
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
  if (agent.configFile === undefined) return false;

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

function isPathWithinRoot(root: string, target: string): boolean {
  const relativePath = relative(root, target);
  return relativePath === "" || (!relativePath.startsWith("..") && !isAbsolute(relativePath));
}

function desiredFileMode(mode: number, executable: boolean): number {
  return executable
    ? mode | ((mode & 0o444) >> 2)
    : mode & ~0o111;
}

async function syncBundledFileMode(path: string, executable: boolean): Promise<boolean> {
  if (process.platform === "win32") {
    return false;
  }

  const stats = await lstat(path);
  if (!stats.isFile()) return false;

  const currentMode = stats.mode & 0o777;
  const nextMode = desiredFileMode(currentMode, executable) & 0o777;
  if (currentMode === nextMode) return false;

  await chmod(path, nextMode);
  return true;
}

async function resolveManagedManifestPath(
  skillDir: string,
  realSkillDir: string,
  relativePath: string,
): Promise<string | undefined> {
  if (relativePath.length === 0 || isAbsolute(relativePath)) {
    return undefined;
  }

  const absolute = resolve(skillDir, relativePath);
  if (!isPathWithinRoot(skillDir, absolute)) {
    return undefined;
  }

  try {
    const realAbsolute = await realpath(absolute);
    return isPathWithinRoot(realSkillDir, realAbsolute) ? absolute : undefined;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return undefined;
    }
    throw error;
  }
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
 * Write a single bundled skill into its install location under the maestro
 * source of truth (`~/.maestro/skills/<skill>/`). Uses a per-skill manifest
 * (`.maestro-bundled.json`) to distinguish user edits from stale content so
 * user edits are preserved across updates.
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

  const perFile = await Promise.all(template.files.map(async (file): Promise<void> => {
    const absolute = join(skillDir, file.path);
    const executable = file.executable === true;
    const shippedHash = contentHash(file.content);
    const existing = await readText(absolute);

    if (existing === undefined) {
      await ensureDir(dirname(absolute));
      await writeText(absolute, file.content);
      await syncBundledFileMode(absolute, executable);
      return { path: file.path, manifestHash: shippedHash, changed: true, preserved: false };
    }

    if (existing === file.content) {
      const modeChanged = await syncBundledFileMode(absolute, executable);
      return { path: file.path, manifestHash: shippedHash, changed: modeChanged, preserved: false };
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
    await syncBundledFileMode(absolute, executable);
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

  const resolvedSkillDir = resolve(skillDir);
  const realSkillDir = await realpath(skillDir).catch((error: unknown) => {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return resolvedSkillDir;
    }
    throw error;
  });
  const preservedUserEdits: string[] = [];
  let changed = false;
  for (const [relativePath, prevHash] of Object.entries(prevManifest.fileHashes)) {
    if (currentPaths.has(relativePath)) continue;

    const absolute = await resolveManagedManifestPath(resolvedSkillDir, realSkillDir, relativePath);
    if (!absolute) {
      preservedUserEdits.push(relativePath);
      continue;
    }

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
 * Remove any maestro-managed skill directory under `skillRoot` that is no
 * longer in the current bundled template set. A skill dir is maestro-managed
 * iff it contains the `.maestro-bundled.json` manifest. User-authored
 * directories sharing the `maestro-` prefix are left untouched.
 */
async function removeStaleBundledSkillDirs(skillRoot: string): Promise<string[]> {
  if (!(await dirExists(skillRoot))) return [];

  const entries = (await readdir(skillRoot, { withFileTypes: true }))
    .sort((left, right) => left.name.localeCompare(right.name));

  const results = await Promise.all(entries.map(async (entry): Promise<void> => {
    if (!entry.isDirectory()) return undefined;
    if (!entry.name.startsWith(BUNDLED_SKILL_PREFIX)) return undefined;
    if (BUNDLED_SKILL_NAME_SET.has(entry.name)) return undefined;

    const manifestPath = join(skillRoot, entry.name, MANIFEST_FILENAME);
    const manifest = await readBundledSkillManifest(manifestPath);
    if (manifest?.managedBy !== "maestro") return undefined;

    await removeIfExists(join(skillRoot, entry.name), { recursive: true });
    return entry.name;
  }));

  return results.filter((name): name is string => name !== undefined);
}

interface EnsureSkillLinkResult {
  readonly changed: boolean;
  readonly preservedUserEdits: readonly string[];
}

/**
 * Ensure `<agentSkillsRoot>/<skillName>` is a directory link pointing to
 * `<maestroSkillsRoot>/<skillName>`. Self-heals on every install/update:
 *
 *   missing                                 -> create link
 *   correct symlink/junction                -> no-op
 *   wrong-target symlink/junction           -> replace
 *   real dir, our manifest, no edits        -> migrate; replace with link
 *   real dir, our manifest, with edits      -> copy edits up; replace with link
 *   real dir, our manifest, divergent edits -> refuse; leave real dir
 *   real dir, no manifest (user-authored)   -> leave; do not link
 *   plain file                              -> refuse; do not clobber
 */
async function ensureSkillLink(
  agentSkillsRoot: string,
  skillName: string,
  maestroSkillsRoot: string,
  entry: Dirent | undefined,
): Promise<EnsureSkillLinkResult> {
  const target = resolve(maestroSkillsRoot, skillName);
  const link = join(agentSkillsRoot, skillName);

  if (entry === undefined) {
    await symlinkDir(target, link);
    return { changed: true, preservedUserEdits: [] };
  }

  if (entry.isSymbolicLink()) {
    const current = await readlinkSafe(link);
    if (current === target) {
      return { changed: false, preservedUserEdits: [] };
    }
    // A symlink that points outside the maestro tree is a user-authored
    // override (e.g. linking our skill name to their own local fork).
    // Replacing it would silently destroy that override. Leave it alone.
    if (current !== undefined && !isMaestroTreeLink(current, maestroSkillsRoot)) {
      const maestroSkillsDisplayPath = formatPathUnderRoot(maestroSkillsRoot, "");
      process.stderr.write(
        `[warn] ${link} is a user symlink pointing outside ${maestroSkillsDisplayPath} (${current}); leaving in place\n`,
      );
      return { changed: false, preservedUserEdits: [] };
    }
    await removeIfExists(link);
    await symlinkDir(target, link);
    return { changed: true, preservedUserEdits: [] };
  }

  if (entry.isDirectory()) {
    return migrateRealDirToSymlink(link, target, skillName);
  }

  process.stderr.write(
    `[error] expected directory link at ${link} but found a non-directory entry; skipping ${skillName}\n`,
  );
  return { changed: false, preservedUserEdits: [] };
}

/**
 * Replace a pre-redesign maestro-managed real directory with a symlink into
 * `~/.maestro/skills/<skill>`. Any user-edited files (relative to the recorded
 * manifest hashes) are first copied into the maestro tree so they survive.
 *
 * If the maestro tree already holds a different value for a user-edited file
 * (e.g. another agent migrated a different edit earlier in this run), we
 * refuse to migrate this skill: the real dir is left in place, an error is
 * surfaced, and the user reconciles before re-running install.
 */
async function migrateRealDirToSymlink(
  realDirPath: string,
  target: string,
  skillName: string,
): Promise<EnsureSkillLinkResult> {
  const manifestPath = join(realDirPath, MANIFEST_FILENAME);
  const manifest = await readBundledSkillManifest(manifestPath);

  if (manifest?.managedBy !== "maestro") {
    // User-authored skill dir that happens to share a name with one we now
    // ship. Don't touch it; don't create a link over it.
    process.stderr.write(
      `[warn] user-authored skill dir at ${realDirPath} — skipping symlink creation for ${skillName}\n`,
    );
    return { changed: false, preservedUserEdits: [] };
  }

  const resolvedRealDir = resolve(realDirPath);
  const realSkillDirPath = await realpath(realDirPath).catch((error: unknown) => {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return resolvedRealDir;
    throw error;
  });

  const userEditCandidates = await Promise.all(
    Object.entries(manifest.fileHashes).map(async ([relativePath, prevHash]): Promise<void> => {
      const absolute = await resolveManagedManifestPath(resolvedRealDir, realSkillDirPath, relativePath);
      if (!absolute) return undefined;
      const existing = await readText(absolute);
      if (existing === undefined) return undefined;
      if (contentHash(existing) === prevHash) return undefined;
      return { relativePath, content: existing };
    }),
  );
  const userEdits = userEditCandidates.filter((e): e is { relativePath: string; content: string } => e !== undefined);

  // Divergence detection: compare each user edit against the maestro tree's
  // current value. If maestro tree differs from the agent's edit AND from the
  // shipped baseline (prevHash), another agent has already migrated a
  // conflicting edit in this run.
  const divergenceChecks = await Promise.all(userEdits.map(async (edit): Promise<void> => {
    const targetFile = join(target, edit.relativePath);
    const existingInMaestro = await readText(targetFile);
    if (existingInMaestro === undefined) return undefined;
    if (existingInMaestro === edit.content) return undefined;
    const prevHash = manifest.fileHashes[edit.relativePath];
    if (contentHash(existingInMaestro) === prevHash) return undefined;
    return edit.relativePath;
  }));
  const conflicts = divergenceChecks.filter((p): p is string => p !== undefined);

  if (conflicts.length > 0) {
    process.stderr.write(
      `[error] divergent user edits for ${skillName} across agents: ${conflicts.join(", ")}. ` +
        `Leaving ${realDirPath} in place; reconcile and re-run \`maestro install\`.\n`,
    );
    return {
      changed: false,
      preservedUserEdits: conflicts.map((path) => `${skillName}/${path}`),
    };
  }

  // Apply user edits to the maestro tree, then replace the real dir with a
  // symlink. The maestro-tree manifest still records the shipped hash for
  // these paths — the existing preservation logic in writeBundledSkill keeps
  // them preserved on subsequent runs (existing != shipped, hash != prev).
  for (const edit of userEdits) {
    const targetFile = join(target, edit.relativePath);
    await ensureDir(dirname(targetFile));
    await writeText(targetFile, edit.content);
  }

  await removeIfExists(realDirPath, { recursive: true });
  await symlinkDir(target, realDirPath);

  return {
    changed: true,
    preservedUserEdits: userEdits.map((e) => `${skillName}/${e.relativePath}`),
  };
}

interface ManagedSkillEntry {
  readonly name: string;
  readonly path: string;
  readonly isSymlink: boolean;
}

/**
 * Classify a single agent-skills directory entry. Returns a `ManagedSkillEntry`
 * iff the entry is one of:
 *   - a symlink whose target lives under `maestroSkillsRoot` (post-redesign install)
 *   - a real dir carrying our `.maestro-bundled.json` manifest (legacy install)
 * Anything else (user dir, foreign symlink, plain file) returns undefined and
 * is left alone by both the install-time stale sweep and uninstall.
 */
async function classifyAgentSkillEntry(
  entry: Dirent,
  entryPath: string,
  maestroSkillsRoot: string,
): Promise<ManagedSkillEntry | undefined> {
  if (!entry.name.startsWith(BUNDLED_SKILL_PREFIX)) return undefined;

  if (entry.isSymbolicLink()) {
    const target = await readlinkSafe(entryPath);
    if (target && isMaestroTreeLink(target, maestroSkillsRoot)) {
      return { name: entry.name, path: entryPath, isSymlink: true };
    }
    return undefined;
  }

  if (entry.isDirectory()) {
    const manifest = await readBundledSkillManifest(join(entryPath, MANIFEST_FILENAME));
    if (manifest?.managedBy === "maestro") {
      return { name: entry.name, path: entryPath, isSymlink: false };
    }
  }

  return undefined;
}

async function removeManagedSkillEntry(entry: ManagedSkillEntry): Promise<boolean> {
  return entry.isSymlink
    ? removeIfExists(entry.path)
    : removeIfExists(entry.path, { recursive: true });
}

/**
 * Sweep maestro-managed entries no longer in the current bundle, given the
 * pre-read directory entries. Symlinks pointing outside the maestro tree and
 * unmanaged real dirs are left alone.
 */
async function sweepStaleAgentSkillEntries(
  agentSkillsRoot: string,
  entries: readonly Dirent[],
  maestroSkillsRoot: string,
): Promise<string[]> {
  const candidates = entries.filter((e) => !BUNDLED_SKILL_NAME_SET.has(e.name));
  const classifications = await Promise.all(
    candidates.map((entry) =>
      classifyAgentSkillEntry(entry, join(agentSkillsRoot, entry.name), maestroSkillsRoot),
    ),
  );
  const stale = classifications.filter((c): c is ManagedSkillEntry => c !== undefined);
  await Promise.all(stale.map(removeManagedSkillEntry));
  return stale.map((s) => s.name);
}

async function processInject(
  agent: AgentConfigSpec,
  projectDir: string,
  homeDir: string,
  maestroSkillsRoot: string,
  globalChanged: boolean,
): Promise<InjectResult> {
  const configDir = agentConfigDirPath(agent, projectDir, homeDir);
  const skillsRoot = agentSkillsRoot(agent, projectDir, homeDir);

  if (!agent.alwaysDetected && !(await dirExists(configDir))) {
    return {
      agent: agent.displayName,
      action: "not-detected",
      configPath: configDir,
      installedSkills: [],
      preservedUserEdits: [],
    };
  }

  await ensureDir(configDir);
  await ensureDir(skillsRoot);

  const dirents = await readdir(skillsRoot, { withFileTypes: true });
  const direntByName = new Map(dirents.map((e) => [e.name, e]));

  // Per-skill links are independent (each touches a distinct maestro-tree
  // subdirectory) so we run them in parallel. Cross-agent divergence is
  // serialized at the injectAgentBlocks level by running agents sequentially.
  const linkResults = await Promise.all(
    BUNDLED_SKILL_NAMES.map((name) =>
      ensureSkillLink(skillsRoot, name, maestroSkillsRoot, direntByName.get(name)),
    ),
  );

  const removedStale = await sweepStaleAgentSkillEntries(skillsRoot, dirents, maestroSkillsRoot);
  const hadLegacy = await cleanupLegacyMaestroMd(agent, projectDir, homeDir);

  const localChanged = linkResults.some((r) => r.changed) || removedStale.length > 0;
  const providerChanged = agent.slug === "hermes"
    ? await ensureHermesExternalDirs(agent, projectDir, homeDir)
    : false;
  const localPreservedEdits = linkResults.flatMap((r) => [...r.preservedUserEdits]);

  for (const path of localPreservedEdits) {
    process.stderr.write(
      `[warn] preserved user-edited skill file: ${formatPathUnderRoot(skillsRoot, path)} (not overwritten)\n`,
    );
  }

  const action: InjectResult["action"] = hadLegacy
    ? "migrated-to-skills"
    : globalChanged || localChanged || providerChanged
      ? "installed"
      : "skipped";

  return {
    agent: agent.displayName,
    action,
    configPath: configDir,
    installedSkills: BUNDLED_SKILL_NAMES,
    preservedUserEdits: localPreservedEdits,
  };
}

async function processRemove(
  agent: AgentConfigSpec,
  projectDir: string,
  homeDir: string,
  maestroSkillsRoot: string,
): Promise<RemoveResult> {
  const configDir = agentConfigDirPath(agent, projectDir, homeDir);
  const skillsRoot = agentSkillsRoot(agent, projectDir, homeDir);

  if (!agent.alwaysDetected && !(await dirExists(configDir))) {
    return {
      agent: agent.displayName,
      action: "not-detected",
      configPath: configDir,
      removedSkills: [],
    };
  }

  const [managedEntries, legacyRemoved] = await Promise.all([
    listManagedSkillEntries(skillsRoot, maestroSkillsRoot),
    cleanupLegacyMaestroMd(agent, projectDir, homeDir),
  ]);

  const removed = await Promise.all(managedEntries.map(async (entry): Promise<void> =>
    (await removeManagedSkillEntry(entry)) ? entry.name : undefined,
  ));
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
 * Enumerate every maestro-managed skill entry under `skillsRoot`. Used by
 * `maestro uninstall`; matches the same classification rules as the install
 * stale sweep, just without the "not in current bundle" filter.
 */
async function listManagedSkillEntries(
  skillsRoot: string,
  maestroSkillsRoot: string,
): Promise<ManagedSkillEntry[]> {
  if (!(await dirExists(skillsRoot))) return [];
  const entries = (await readdir(skillsRoot, { withFileTypes: true }))
    .sort((left, right) => left.name.localeCompare(right.name));
  const results = await Promise.all(entries.map((entry) =>
    classifyAgentSkillEntry(entry, join(skillsRoot, entry.name), maestroSkillsRoot),
  ));
  return results.filter((entry): entry is ManagedSkillEntry => entry !== undefined);
}

export async function injectAgentBlocks(
  projectDir = process.cwd(),
  targetScope: AgentConfigTargetScope = "all",
  homeDir?: string,
  providerIds?: readonly AgentConfigSpec["providerId"][],
): Promise<InjectResult[]> {
  const resolvedHomeDir = homeDir ?? homedir();
  const maestroSkillsRoot = resolveMaestroSkillsRoot(resolvedHomeDir);
  const providerFilter = providerIds ? new Set(providerIds) : undefined;

  await ensureDir(maestroSkillsRoot);

  // Stale-dir removal and per-skill writes touch disjoint paths (stale dirs
  // are by definition not in BUNDLED_SKILL_NAMES), so they're safe to run
  // concurrently.
  const [removedStaleMaestro, writeResults] = await Promise.all([
    removeStaleBundledSkillDirs(maestroSkillsRoot),
    Promise.all(BUNDLED_SKILL_TEMPLATES.map((t) => writeBundledSkill(maestroSkillsRoot, t))),
  ]);
  const globalChanged = removedStaleMaestro.length > 0 || writeResults.some((r) => r.changed);

  for (const r of writeResults) {
    for (const path of r.preservedUserEdits) {
      process.stderr.write(
        `[warn] preserved user-edited skill file: ${formatPathUnderRoot(maestroSkillsRoot, path)} (not overwritten)\n`,
      );
    }
  }

  // Run agents sequentially so cross-agent divergence detection during
  // migration sees the maestro tree state left by the prior agent — Promise.all
  // would race two migrations against the same maestro-tree path.
  const results: InjectResult[] = [];
  for (const agent of SKILL_TARGET_AGENTS.filter((a) =>
    agentMatchesTargetScope(a, targetScope) && (providerFilter?.has(a.providerId) ?? true)
  )) {
    results.push(await processInject(agent, projectDir, resolvedHomeDir, maestroSkillsRoot, globalChanged));
  }
  return results;
}

export async function removeAgentBlocks(
  projectDir = process.cwd(),
  targetScope: AgentConfigTargetScope = "all",
  homeDir?: string,
  providerIds?: readonly AgentConfigSpec["providerId"][],
): Promise<RemoveResult[]> {
  const resolvedHomeDir = homeDir ?? homedir();
  const maestroSkillsRoot = resolveMaestroSkillsRoot(resolvedHomeDir);
  const providerFilter = providerIds ? new Set(providerIds) : undefined;
  return Promise.all(
    SKILL_TARGET_AGENTS
      .filter((agent) => agentMatchesTargetScope(agent, targetScope) && (providerFilter?.has(agent.providerId) ?? true))
      .map((agent) => processRemove(agent, projectDir, resolvedHomeDir, maestroSkillsRoot)),
  );
}

export { agentLegacyConfigPaths };

async function ensureHermesExternalDirs(
  agent: AgentConfigSpec,
  projectDir: string,
  homeDir: string,
): Promise<boolean> {
  const configPath = agentConfigPath(agent, projectDir, homeDir);
  const sharedRoot = resolveAgentSkillsSharedRoot(homeDir);
  const raw = await readText(configPath);
  const parsed = raw?.trim()
    ? parseYaml<Record<string, unknown>>(raw)
    : {};
  const skills = isRecord(parsed.skills) ? { ...parsed.skills } : {};
  const current = Array.isArray(skills.external_dirs)
    ? skills.external_dirs.filter((entry): entry is string => typeof entry === "string")
    : [];
  if (current.includes(sharedRoot)) return false;

  skills.external_dirs = [...current, sharedRoot];
  const next = stringifyYaml({ ...parsed, skills });
  await ensureDir(dirname(configPath));
  if (raw !== undefined) {
    await copyFile(configPath, `${configPath}.bak-${Date.now()}`);
  }
  await writeText(configPath, next);
  return true;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
