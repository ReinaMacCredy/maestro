import type { Command } from "commander";
import { createHash } from "node:crypto";
import { homedir } from "node:os";
import { cp, mkdtemp, readdir, realpath, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, extname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { parseYaml } from "@/shared/lib/yaml.js";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag, warn } from "@/shared/lib/output.js";
import { resolveWithin } from "@/shared/lib/path-safety.js";
import {
  dirExists,
  ensureDir,
  listFilesRecursive,
  readText,
  readlinkSafe,
  removeIfExists,
  symlinkDir,
  writeJson,
} from "@/shared/lib/fs.js";
import {
  resolveAgentSkillsSharedRoot,
  resolveMaestroExternalSkillsRoot,
} from "@/shared/domain/defaults.js";
import { execArgv } from "@/shared/lib/shell.js";
import { listSkillTargetProviders, type ProviderId } from "@/infra/domain/providers.js";
import { injectAgentBlocks } from "@/infra/usecases/manage-agents.usecase.js";

export type SkillDiscoveryScope = "project" | "user" | "shared" | "all";
export type SkillInstallScope = "project" | "user" | "shared";
type SkillTarget = ProviderId | "all";

export interface SkillRecord {
  readonly name: string;
  readonly description: string;
  readonly path: string;
  readonly root: string;
  readonly scope: SkillDiscoveryScope | "builtin" | "provider";
  readonly source: string;
  readonly metadata: Record<string, unknown>;
  readonly body: string;
}

/**
 * Lean projection of {@link SkillRecord} for `skills list` JSON output.
 * The full SKILL.md `body` is the dominant byte source (~1 MB on the maestro
 * repo); list endpoints drop it. `skills inspect <name>` still returns the
 * full record. `--full` on `skills list` recovers the old shape.
 */
export interface SkillSummary {
  readonly name: string;
  readonly description: string;
  readonly scope: SkillRecord["scope"];
  readonly source: string;
  readonly path: string;
}

function summarizeSkill(skill: SkillRecord): SkillSummary {
  return {
    name: skill.name,
    description: skill.description,
    scope: skill.scope,
    source: skill.source,
    path: skill.path,
  };
}

export interface SkillDiagnostic {
  readonly level: "warning" | "error";
  readonly message: string;
  readonly path?: string;
}

export interface SkillInstallResult {
  readonly name: string;
  readonly scope: SkillInstallScope;
  readonly root: string;
  readonly installedTargets: readonly ProviderId[];
  readonly manifestPath: string;
}

interface ManagedSkillManifest {
  readonly managedBy: "maestro";
  readonly kind: "external-skill";
  readonly name: string;
  readonly source: string;
  readonly resolvedSource?: string;
  readonly fileHashes: Record<string, string>;
  readonly installedTargetRoots: readonly string[];
  readonly installedAt: string;
}

const SKILL_FILE = "SKILL.md";
const EXTERNAL_MANIFEST = ".maestro-external-skill.json";

export function registerSkillsCommand(program: Command): void {
  const skills = program
    .command("skills")
    .description("Discover, inspect, install, remove, and sync AgentSkills-compatible skills");

  skills
    .command("list")
    .option("--scope <scope>", "project|user|shared|all", "all")
    .option("--full", "Include SKILL.md body in JSON output (verbose; default summary)")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const isJson = resolveJsonFlag(opts, program);
      const scope = parseScope(opts.scope);
      const isFull = opts.full === true;
      const result = await discoverSkills({ cwd: process.cwd(), homeDir: homedir(), scope });
      if (isJson) {
        const projected = {
          skills: isFull ? result.skills : result.skills.map(summarizeSkill),
          diagnostics: result.diagnostics,
        };
        output(true, projected, formatSkillDiscoveryResult);
        return;
      }
      for (const diagnostic of result.diagnostics) {
        if (diagnostic.level === "warning") warn(diagnostic.message);
      }
      output(isJson, result.skills, formatSkillList);
    });

  skills
    .command("inspect <name>")
    .option("--json", "Output as JSON")
    .action(async (name: string, opts): Promise<void> => {
      const isJson = resolveJsonFlag(opts, program);
      const result = await discoverSkills({ cwd: process.cwd(), homeDir: homedir(), scope: "all" });
      const skill = result.skills.find((candidate) => candidate.name === name);
      if (!skill) {
        throw new MaestroError(`Skill not found: ${name}`, [
          "Run `maestro skills list --scope all` to see discovered skills",
        ]);
      }
      output(isJson, skill, formatSkillInspect);
    });

  skills
    .command("install <source>")
    .option("--scope <scope>", "user|project|shared", "user")
    .option("--targets <targets>", "all or comma-separated codex,claude,hermes,agentskills", "all")
    .option("--json", "Output as JSON")
    .action(async (source: string, opts): Promise<void> => {
      const isJson = resolveJsonFlag(opts, program);
      const scope = parseInstallScope(opts.scope);
      const targets = parseTargets(opts.targets);
      const results = await installSkillSource({
        source,
        scope,
        targets,
        cwd: process.cwd(),
        homeDir: homedir(),
      });
      output(isJson, results, formatInstallResults);
    });

  skills
    .command("remove <name>")
    .option("--scope <scope>", "user|project|shared", "user")
    .option("--json", "Output as JSON")
    .action(async (name: string, opts): Promise<void> => {
      const isJson = resolveJsonFlag(opts, program);
      const scope = parseInstallScope(opts.scope);
      const result = await removeManagedSkill({ name, scope, cwd: process.cwd(), homeDir: homedir() });
      output(isJson, result, (r) => [
        r.removed
          ? `[ok] Removed ${name} from ${scope}`
          : `[--] No managed skill named ${name} found in ${scope}`,
      ]);
    });

  skills
    .command("sync")
    .option("--targets <targets>", "all or comma-separated codex,claude,hermes,agentskills", "all")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const isJson = resolveJsonFlag(opts, program);
      const targets = parseTargets(opts.targets);
      const bundled = await injectAgentBlocks(
        process.cwd(),
        "home",
        homedir(),
        targets.includes("all") ? undefined : targets as readonly ProviderId[],
      );
      const external = await syncManagedSkillsToTargets({
        cwd: process.cwd(),
        homeDir: homedir(),
        targets,
      });
      output(isJson, { bundled, external }, (r) => [
        `[ok] Synced bundled skills to ${r.bundled.length} target(s)`,
        `[ok] Synced ${r.external.length} external skill(s)`,
      ]);
    });
}

export function parseSkillMarkdown(
  content: string,
  skillDirName: string,
): { skill?: Omit<SkillRecord, "path" | "root" | "scope" | "source">; diagnostics: SkillDiagnostic[] } {
  const diagnostics: SkillDiagnostic[] = [];
  if (!content.startsWith("---")) {
    return {
      diagnostics: [{ level: "error", message: "SKILL.md is missing YAML frontmatter" }],
    };
  }
  const close = content.indexOf("\n---", 3);
  if (close === -1) {
    return {
      diagnostics: [{ level: "error", message: "SKILL.md frontmatter is not closed" }],
    };
  }

  const rawFrontmatter = content.slice(3, close).trim();
  const body = content.slice(content.indexOf("\n", close + 1) + 1).trim();
  let frontmatter: Record<string, unknown>;
  try {
    frontmatter = parseYaml<Record<string, unknown>>(rawFrontmatter) ?? {};
  } catch (error) {
    return {
      diagnostics: [{
        level: "error",
        message: `SKILL.md frontmatter is malformed: ${error instanceof Error ? error.message : String(error)}`,
      }],
    };
  }

  const name = typeof frontmatter.name === "string" ? frontmatter.name.trim() : "";
  const description = typeof frontmatter.description === "string" ? frontmatter.description.trim() : "";
  if (name.length === 0) {
    diagnostics.push({ level: "error", message: "SKILL.md frontmatter requires name" });
  }
  if (description.length === 0) {
    diagnostics.push({ level: "error", message: "SKILL.md frontmatter requires description" });
  }
  if (name && name !== skillDirName) {
    diagnostics.push({
      level: "warning",
      message: `Skill name '${name}' does not match directory '${skillDirName}'`,
    });
  }
  if (!/^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$/.test(name) || name.includes("--")) {
    diagnostics.push({
      level: "warning",
      message: `Skill name '${name}' does not follow Agent Skills naming constraints`,
    });
  }

  const metadata = { ...frontmatter };
  delete metadata.name;
  delete metadata.description;

  if (diagnostics.some((d) => d.level === "error")) {
    return { diagnostics };
  }

  return {
    skill: { name, description, metadata, body },
    diagnostics,
  };
}

export async function discoverSkills(input: {
  readonly cwd: string;
  readonly homeDir: string;
  readonly scope: SkillDiscoveryScope;
}): Promise<{ skills: readonly SkillRecord[]; diagnostics: readonly SkillDiagnostic[] }> {
  const roots = discoveryRoots(input.cwd, input.homeDir, input.scope);
  const diagnostics: SkillDiagnostic[] = [];
  const selected = new Map<string, SkillRecord>();

  for (const root of roots) {
    if (!await dirExists(root.path)) continue;
    const entries = (await readdir(root.path, { withFileTypes: true }))
      .filter((entry) => entry.isDirectory() || entry.isSymbolicLink())
      .sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const skillPath = join(root.path, entry.name, SKILL_FILE);
      const raw = await readText(skillPath);
      if (raw === undefined) continue;
      const parsed = parseSkillMarkdown(raw, entry.name);
      diagnostics.push(...parsed.diagnostics.map((d) => ({ ...d, path: skillPath })));
      if (!parsed.skill) continue;
      const record: SkillRecord = {
        ...parsed.skill,
        path: join(root.path, entry.name),
        root: root.path,
        scope: root.scope,
        source: root.label,
      };
      if (selected.has(record.name)) {
        diagnostics.push({
          level: "warning",
          message: `Skill '${record.name}' from ${record.path} is shadowed by ${selected.get(record.name)!.path}`,
          path: record.path,
        });
        continue;
      }
      selected.set(record.name, record);
    }
  }

  return {
    skills: [...selected.values()].sort((left, right) => left.name.localeCompare(right.name)),
    diagnostics,
  };
}

export async function installSkillSource(input: {
  readonly source: string;
  readonly scope: SkillInstallScope;
  readonly targets: readonly SkillTarget[];
  readonly cwd: string;
  readonly homeDir: string;
}): Promise<readonly SkillInstallResult[]> {
  const prepared = await prepareSource(input.source, input.cwd);
  try {
    const candidates = await findSkillDirectories(prepared.path);
    if (candidates.length === 0) {
      throw new MaestroError(`No Agent Skill directories found in ${input.source}`, [
        "A skill directory must contain SKILL.md with name and description frontmatter",
      ]);
    }
    const managedRoot = installRoot(input.scope, input.cwd, input.homeDir);
    await ensureDir(managedRoot);
    const results: SkillInstallResult[] = [];
    for (const candidate of candidates) {
      const raw = await readText(join(candidate, SKILL_FILE));
      if (raw === undefined) continue;
      const parsed = parseSkillMarkdown(raw, basename(candidate));
      if (!parsed.skill) {
        throw new MaestroError(`Invalid skill at ${candidate}`, parsed.diagnostics.map((d) => d.message));
      }
      const targetDir = resolveWithin(managedRoot, parsed.skill.name, "Skill install path");
      await removeIfExists(targetDir, { recursive: true });
      await cp(candidate, targetDir, { recursive: true });
      const installedTargets = await syncOneSkillToTargets(
        targetDir,
        parsed.skill.name,
        input.targets,
        input.cwd,
        input.homeDir,
      );
      const manifestPath = join(targetDir, EXTERNAL_MANIFEST);
      const manifest: ManagedSkillManifest = {
        managedBy: "maestro",
        kind: "external-skill",
        name: parsed.skill.name,
        source: input.source,
        ...(prepared.resolvedSource ? { resolvedSource: prepared.resolvedSource } : {}),
        fileHashes: await hashFiles(targetDir),
        installedTargetRoots: installedTargets.map((target) => target.root),
        installedAt: new Date().toISOString(),
      };
      await writeJson(manifestPath, manifest);
      results.push({
        name: parsed.skill.name,
        scope: input.scope,
        root: targetDir,
        installedTargets: installedTargets.map((target) => target.id),
        manifestPath,
      });
    }
    return results;
  } finally {
    if (prepared.cleanup) await rm(prepared.cleanup, { recursive: true, force: true });
  }
}

export async function syncManagedSkillsToTargets(input: {
  readonly cwd: string;
  readonly homeDir: string;
  readonly targets: readonly SkillTarget[];
}): Promise<readonly SkillInstallResult[]> {
  const roots: Array<{ scope: SkillInstallScope; path: string }> = [
    { scope: "project", path: installRoot("project", input.cwd, input.homeDir) },
    { scope: "user", path: installRoot("user", input.cwd, input.homeDir) },
    { scope: "shared", path: installRoot("shared", input.cwd, input.homeDir) },
  ];
  const results: SkillInstallResult[] = [];
  for (const root of roots) {
    if (!await dirExists(root.path)) continue;
    const entries = (await readdir(root.path, { withFileTypes: true }))
      .filter((entry) => entry.isDirectory() || entry.isSymbolicLink())
      .sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const skillDir = join(root.path, entry.name);
      const raw = await readText(join(skillDir, SKILL_FILE));
      if (raw === undefined) continue;
      const parsed = parseSkillMarkdown(raw, entry.name);
      if (!parsed.skill) continue;
      const installedTargets = await syncOneSkillToTargets(
        skillDir,
        parsed.skill.name,
        input.targets,
        input.cwd,
        input.homeDir,
      );
      const manifestPath = join(skillDir, EXTERNAL_MANIFEST);
      results.push({
        name: parsed.skill.name,
        scope: root.scope,
        root: skillDir,
        installedTargets: installedTargets.map((target) => target.id),
        manifestPath,
      });
    }
  }
  return results;
}

export async function removeManagedSkill(input: {
  readonly name: string;
  readonly scope: SkillInstallScope;
  readonly cwd: string;
  readonly homeDir: string;
}): Promise<{ readonly name: string; readonly removed: boolean; readonly removedTargets: readonly ProviderId[] }> {
  const root = installRoot(input.scope, input.cwd, input.homeDir);
  const sourceDir = resolveWithin(root, input.name, "Skill remove path");
  const manifest = await readManagedManifest(sourceDir);
  const removedTargets: ProviderId[] = [];
  for (const provider of listSkillTargetProviders(input.cwd, input.homeDir)) {
    const target = resolveWithin(provider.skillsRoot, input.name, "Skill target path");
    if (await removeManagedTarget(target, sourceDir)) {
      removedTargets.push(provider.id);
    }
  }
  const removedSource = manifest
    ? await removeIfExists(sourceDir, { recursive: true })
    : false;
  return { name: input.name, removed: removedSource || removedTargets.length > 0, removedTargets };
}

function discoveryRoots(cwd: string, homeDir: string, scope: SkillDiscoveryScope): Array<{
  readonly label: string;
  readonly scope: SkillRecord["scope"];
  readonly path: string;
}> {
  const projectRoots = [
    { label: "project-maestro", scope: "project" as const, path: join(cwd, ".maestro", "skills") },
    { label: "project-agentskills", scope: "project" as const, path: join(cwd, ".agents", "skills") },
  ];
  const builtinRoots = [
    { label: "repo-bundled", scope: "builtin" as const, path: join(cwd, "skills", "bundled") },
  ];
  const userRoots = [
    { label: "maestro-external", scope: "user" as const, path: resolveMaestroExternalSkillsRoot(homeDir) },
  ];
  const sharedRoots = [
    { label: "agentskills", scope: "shared" as const, path: resolveAgentSkillsSharedRoot(homeDir) },
  ];
  const providerRoots = listSkillTargetProviders(cwd, homeDir).map((provider) => ({
    label: provider.id,
    scope: "provider" as const,
    path: provider.skillsRoot,
  }));

  switch (scope) {
    case "project":
      return projectRoots;
    case "user":
      return [...userRoots, ...providerRoots.filter((root) => root.label !== "agentskills")];
    case "shared":
      return sharedRoots;
    case "all":
      return [...projectRoots, ...builtinRoots, ...userRoots, ...sharedRoots, ...providerRoots];
  }
}

function installRoot(scope: SkillInstallScope, cwd: string, homeDir: string): string {
  switch (scope) {
    case "project":
      return join(cwd, ".maestro", "skills");
    case "user":
      return resolveMaestroExternalSkillsRoot(homeDir);
    case "shared":
      return resolveAgentSkillsSharedRoot(homeDir);
  }
}

async function syncOneSkillToTargets(
  sourceDir: string,
  name: string,
  targets: readonly SkillTarget[],
  cwd: string,
  homeDir: string,
): Promise<readonly { readonly id: ProviderId; readonly root: string }[]> {
  const selected = selectTargetProviders(targets, cwd, homeDir);
  const sourceReal = await realpath(sourceDir).catch(() => resolve(sourceDir));
  const installed: Array<{ id: ProviderId; root: string }> = [];
  for (const provider of selected) {
    await ensureDir(provider.skillsRoot);
    const target = resolveWithin(provider.skillsRoot, name, "Skill target path");
    const targetReal = await realpath(target).catch(() => undefined);
    if (targetReal === sourceReal) {
      installed.push({ id: provider.id, root: provider.skillsRoot });
      continue;
    }
    const link = await readlinkSafe(target);
    if (link !== undefined) {
      const resolvedLink = isAbsolute(link) ? link : resolve(dirname(target), link);
      if (resolvedLink === sourceReal || isUnderMaestroManagedRoot(resolvedLink, homeDir)) {
        await removeIfExists(target);
      } else {
        warn(`Leaving user symlink in place: ${target}`);
        continue;
      }
    } else if (await pathExists(target)) {
      const manifest = await readManagedManifest(target);
      if (manifest?.managedBy === "maestro") {
        await removeIfExists(target, { recursive: true });
      } else {
        warn(`Leaving unmanaged skill directory in place: ${target}`);
        continue;
      }
    }
    await symlinkDir(sourceReal, target);
    installed.push({ id: provider.id, root: provider.skillsRoot });
  }
  return installed;
}

function selectTargetProviders(targets: readonly SkillTarget[], cwd: string, homeDir: string) {
  const providers = listSkillTargetProviders(cwd, homeDir);
  if (targets.includes("all")) return providers;
  const ids = new Set(targets);
  return providers.filter((provider) => ids.has(provider.id));
}

async function removeManagedTarget(target: string, sourceDir: string): Promise<boolean> {
  const sourceReal = await realpath(sourceDir).catch(() => resolve(sourceDir));
  const link = await readlinkSafe(target);
  if (link !== undefined) {
    const resolvedLink = isAbsolute(link) ? link : resolve(dirname(target), link);
    if (resolvedLink === sourceReal) return removeIfExists(target);
    return false;
  }
  const manifest = await readManagedManifest(target);
  if (manifest?.managedBy === "maestro") {
    return removeIfExists(target, { recursive: true });
  }
  return false;
}

async function prepareSource(
  source: string,
  cwd: string,
): Promise<{ readonly path: string; readonly resolvedSource?: string; readonly cleanup?: string }> {
  const localSource = isAbsolute(source) ? source : resolve(cwd, source);
  if (await pathExists(localSource)) {
    const absolute = isAbsolute(source) ? source : localSource;
    return { path: absolute };
  }
  if (isGitUrl(source) || isGithubShorthand(source)) {
    const tmp = await mkdtemp(join(tmpdir(), "maestro-skill-git-"));
    const { repo, subpath } = normalizeGitSource(source);
    const clone = await execArgv(["git", "clone", "--depth", "1", repo, tmp], { timeout: 60_000 });
    if (clone.exitCode !== 0) {
      throw new MaestroError(`Failed to clone skill source: ${source}`, [clone.stderr || clone.stdout]);
    }
    const head = await execArgv(["git", "rev-parse", "HEAD"], { cwd: tmp, timeout: 10_000 });
    return {
      path: subpath ? resolveWithin(tmp, subpath, "Skill source subpath") : tmp,
      resolvedSource: head.exitCode === 0 ? `${repo}#${head.stdout.trim()}` : repo,
      cleanup: tmp,
    };
  }
  if (/^https?:\/\//.test(source)) {
    return prepareHttpArchive(source);
  }
  throw new MaestroError(`Unsupported skill source: ${source}`, [
    "Use a local directory, Git URL, GitHub shorthand owner/repo[/path], or HTTP zip/tar archive URL",
    "Marketplace slug lookup is intentionally not implemented without a stable documented registry API",
  ]);
}

async function prepareHttpArchive(source: string): Promise<{ readonly path: string; readonly resolvedSource: string; readonly cleanup: string }> {
  if (!/\.(zip|tar|tgz|tar\.gz)$/i.test(source)) {
    throw new MaestroError(`Unsupported HTTP skill source: ${source}`, [
      "HTTP installs must point to a zip, tar, tgz, or tar.gz archive",
    ]);
  }
  const tmp = await mkdtemp(join(tmpdir(), "maestro-skill-http-"));
  const archive = join(tmp, `archive${archiveExtension(source)}`);
  const response = await fetch(source);
  if (!response.ok) {
    throw new MaestroError(`Failed to download skill archive: HTTP ${response.status}`, [source]);
  }
  // Stream to disk via Bun.write(Response) — avoids loading the full archive
  // into memory, which would scale linearly with archive size.
  await Bun.write(archive, response);
  const extractDir = join(tmp, "extract");
  await ensureDir(extractDir);
  const isZip = archive.endsWith(".zip");

  // Pre-extraction validation: list archive contents and reject any entry
  // with an absolute path or `..` segment. This is the first line of defense
  // against Zip Slip (CWE-22). Extraction itself is the second.
  const listCommand = isZip ? ["unzip", "-Z", "-1", archive] : ["tar", "-tf", archive];
  const list = await execArgv(listCommand, { timeout: 60_000 });
  if (list.exitCode !== 0) {
    throw new MaestroError(`Failed to inspect skill archive: ${source}`, [
      list.stderr || list.stdout,
    ]);
  }
  assertSafeArchiveEntries(list.stdout.split("\n"), source);

  const command = isZip
    ? ["unzip", "-q", archive, "-d", extractDir]
    : ["tar", "-xf", archive, "-C", extractDir];
  const extracted = await execArgv(command, { timeout: 60_000 });
  if (extracted.exitCode !== 0) {
    throw new MaestroError(`Failed to extract skill archive: ${source}`, [extracted.stderr || extracted.stdout]);
  }
  // Post-extraction validation: walk the tree, refuse any symlink or realpath
  // that resolves outside extractDir. Catches symlink-based escapes the
  // pre-extraction listing cannot see.
  await assertExtractedPathsContained(extractDir, source);
  return { path: extractDir, resolvedSource: source, cleanup: tmp };
}

function assertSafeArchiveEntries(entries: readonly string[], source: string): void {
  for (const raw of entries) {
    const entry = raw.trim();
    if (entry.length === 0) continue;
    if (isAbsolute(entry) || entry.startsWith("/") || entry.startsWith("\\") || /^[A-Za-z]:[\\/]/.test(entry)) {
      throw new MaestroError(`Refusing to install ${source}: archive contains absolute path '${entry}'`, [
        "Archive may be malicious (Zip Slip / CWE-22 directory traversal).",
      ]);
    }
    const segments = entry.split(/[\\/]+/);
    if (segments.some((segment) => segment === "..")) {
      throw new MaestroError(`Refusing to install ${source}: archive contains parent-directory traversal in '${entry}'`, [
        "Archive may be malicious (Zip Slip / CWE-22 directory traversal).",
      ]);
    }
  }
}

async function assertExtractedPathsContained(extractDir: string, source: string): Promise<void> {
  const realRoot = await realpath(extractDir);
  const stack: string[] = [extractDir];
  while (stack.length > 0) {
    const current = stack.pop()!;
    const entries = await readdir(current, { withFileTypes: true });
    for (const entry of entries) {
      const child = join(current, entry.name);
      if (entry.isSymbolicLink()) {
        const link = await readlinkSafe(child);
        if (link === undefined) continue;
        const resolved = isAbsolute(link) ? link : resolve(dirname(child), link);
        if (!isPathWithin(resolved, realRoot)) {
          throw new MaestroError(`Refusing to install ${source}: extracted symlink '${relative(extractDir, child)}' points outside the archive root (${link})`, [
            "Archive may be malicious (Zip Slip / CWE-22 symlink escape).",
          ]);
        }
        continue;
      }
      const real = await realpath(child).catch(() => resolve(child));
      if (!isPathWithin(real, realRoot)) {
        throw new MaestroError(`Refusing to install ${source}: extracted entry '${relative(extractDir, child)}' resolves outside the archive root`, [
          "Archive may be malicious (Zip Slip / CWE-22 directory traversal).",
        ]);
      }
      if (entry.isDirectory()) stack.push(child);
    }
  }
}

function isPathWithin(candidate: string, root: string): boolean {
  const resolvedCandidate = resolve(candidate);
  const resolvedRoot = resolve(root);
  return resolvedCandidate === resolvedRoot || resolvedCandidate.startsWith(resolvedRoot + sep);
}

async function findSkillDirectories(root: string): Promise<readonly string[]> {
  const stats = await stat(root).catch(() => undefined);
  if (!stats?.isDirectory()) return [];
  if (await readText(join(root, SKILL_FILE)) !== undefined) return [root];
  const entries = (await readdir(root, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory() || entry.isSymbolicLink())
    .sort((left, right) => left.name.localeCompare(right.name));
  const dirs: string[] = [];
  for (const entry of entries) {
    const candidate = join(root, entry.name);
    if (await readText(join(candidate, SKILL_FILE)) !== undefined) dirs.push(candidate);
  }
  return dirs;
}

async function hashFiles(root: string): Promise<Record<string, string>> {
  const files = await listFilesRecursive(root);
  const hashes: Record<string, string> = {};
  for (const file of files) {
    const rel = relative(root, file);
    if (rel === EXTERNAL_MANIFEST) continue;
    const content = await readText(file);
    if (content !== undefined) {
      hashes[rel] = createHash("sha256").update(content).digest("hex");
    }
  }
  return hashes;
}

async function readManagedManifest(skillDir: string): Promise<ManagedSkillManifest | undefined> {
  const raw = await readText(join(skillDir, EXTERNAL_MANIFEST));
  if (raw === undefined) return undefined;
  try {
    return JSON.parse(raw) as ManagedSkillManifest;
  } catch {
    return undefined;
  }
}

function isUnderMaestroManagedRoot(path: string, homeDir: string): boolean {
  const roots = [
    resolveMaestroExternalSkillsRoot(homeDir),
    resolveAgentSkillsSharedRoot(homeDir),
  ].map((root) => resolve(root));
  const resolved = resolve(path);
  return roots.some((root) => resolved === root || resolved.startsWith(root + sep));
}

function parseScope(value: unknown): SkillDiscoveryScope {
  if (value === "project" || value === "user" || value === "shared" || value === "all") return value;
  throw new MaestroError(`Invalid --scope '${String(value)}'`, ["Valid scopes: project, user, shared, all"]);
}

function parseInstallScope(value: unknown): SkillInstallScope {
  if (value === "project" || value === "user" || value === "shared") return value;
  throw new MaestroError(`Invalid --scope '${String(value)}'`, ["Valid scopes: user, project, shared"]);
}

function parseTargets(value: unknown): readonly SkillTarget[] {
  const raw = typeof value === "string" ? value : "all";
  const targets = raw.split(",").map((part) => part.trim()).filter(Boolean);
  if (targets.length === 0 || targets.includes("all")) return ["all"];
  for (const target of targets) {
    if (target !== "codex" && target !== "claude" && target !== "hermes" && target !== "agentskills") {
      throw new MaestroError(`Invalid --targets entry '${target}'`, [
        "Use all or comma-separated codex,claude,hermes,agentskills",
      ]);
    }
  }
  return targets as ProviderId[];
}

function isGitUrl(source: string): boolean {
  return /^git@/.test(source) || /\.git(?:#.+)?$/.test(source) || /^https?:\/\/.*\.git(?:#.+)?$/.test(source);
}

function isGithubShorthand(source: string): boolean {
  return /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+(?:\/[A-Za-z0-9_.\/-]+)?$/.test(source);
}

function normalizeGitSource(source: string): { readonly repo: string; readonly subpath?: string } {
  if (isGitUrl(source)) return { repo: source };
  const parts = source.split("/");
  const owner = parts[0]!;
  const repo = parts[1]!;
  const subpathParts = parts.slice(2);
  if (subpathParts.some((segment) => segment === "" || segment === "." || segment === "..")) {
    throw new MaestroError(`Invalid skill source subpath in '${source}'`, [
      "GitHub shorthand subpaths must not contain '.' or '..' segments",
    ]);
  }
  const subpath = subpathParts.join("/");
  return {
    repo: `https://github.com/${owner}/${repo}.git`,
    ...(subpath ? { subpath } : {}),
  };
}

function archiveExtension(source: string): string {
  if (source.endsWith(".tar.gz")) return ".tar.gz";
  const ext = extname(new URL(source).pathname);
  return ext || ".archive";
}

async function pathExists(path: string): Promise<boolean> {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

function formatSkillList(skills: readonly SkillRecord[]): string[] {
  if (skills.length === 0) return ["No skills found"];
  return [
    `[ok] ${skills.length} skill(s)`,
    ...skills.map((skill) => `  ${skill.name}  ${skill.scope}  ${skill.path}`),
  ];
}

function formatSkillDiscoveryResult(result: {
  readonly skills: readonly (SkillRecord | SkillSummary)[];
  readonly diagnostics: readonly SkillDiagnostic[];
}): string[] {
  if (result.skills.length === 0) return ["No skills found"];
  return [
    `[ok] ${result.skills.length} skill(s)`,
    ...result.skills.map((skill) => `  ${skill.name}  ${skill.scope}  ${skill.path}`),
  ];
}

function formatSkillInspect(skill: SkillRecord): string[] {
  return [
    `[ok] ${skill.name}`,
    `  Description: ${skill.description}`,
    `  Scope: ${skill.scope}`,
    `  Path: ${skill.path}`,
    `  Source: ${skill.source}`,
  ];
}

function formatInstallResults(results: readonly SkillInstallResult[]): string[] {
  return [
    `[ok] Installed ${results.length} skill(s)`,
    ...results.map((result) =>
      `  ${result.name}  scope=${result.scope}  targets=${result.installedTargets.join(",") || "(none)"}`
    ),
  ];
}
