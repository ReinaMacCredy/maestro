import type { ConfigPort } from "../ports/config.port.js";
import { DEFAULT_CONFIG } from "@/infra/domain/config-types.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import {
  PROJECT_BOOTSTRAP_TEMPLATES,
  type BootstrapTemplateFile,
} from "../domain/bootstrap-templates.js";
import {
  BUILT_IN_SKILL_TEMPLATES,
  type BuiltInSkillTemplate,
} from "../domain/built-in-skill-templates.js";
import { dirExists, ensureDir, listFilesRecursive, readText, writeText } from "@/shared/lib/fs.js";
import {
  isManagedSkillDirectoryName,
  resolveSkillDirectoryName,
} from "@/shared/lib/skill-path.js";
import { homedir } from "node:os";
import { dirname, join, posix, relative, resolve, sep } from "node:path";
import { chmod, lstat, readdir, rm } from "node:fs/promises";
import { DEFAULT_PRINCIPLES } from "@/v2/service/default-principles.js";

const RUNTIME_GITIGNORE_COMMENT = "# Maestro runtime state";
const RUNTIME_GITIGNORE_LINES = [
  ".maestro/missions/",
  ".maestro/sessions/",
  ".maestro/tasks/local-history/",
  ".maestro/evidence/",
  ".maestro/runs/",
] as const;
const MANAGED_AGENT_SKILL_ROOTS = [
  [".claude", "skills"],
  [".codex", "skills"],
] as const;

export interface InitResult {
  readonly created: string[];
  readonly skipped: string[];
  readonly scope: "global" | "project";
  readonly bootstrapGenerated: boolean;
}

export async function initMaestro(
  config: ConfigPort,
  opts: {
    global: boolean;
    dir: string;
    confirmReplace?: (path: string) => Promise<boolean>;
  },
): Promise<InitResult> {
  const scope = opts.global ? "global" : "project";
  const created: string[] = [];
  const skipped: string[] = [];

  if (opts.global) {
    const globalDir = join(homedir(), MAESTRO_DIR);
    await ensureDirIfMissing(globalDir, created);

    if (!(await config.exists("global", opts.dir))) {
      await config.write("global", opts.dir, DEFAULT_CONFIG);
      created.push(join(globalDir, "config.yaml"));
    } else {
      skipped.push(join(globalDir, "config.yaml"));
    }
  } else {
    const maestroDir = join(opts.dir, MAESTRO_DIR);
    const tasksDir = join(maestroDir, "tasks");
    const continuationDir = join(tasksDir, "continuations");
    const activeContinuationDir = join(continuationDir, "active");
    const completedContinuationDir = join(continuationDir, "completed");
    const taskLocalHistoryDir = join(tasksDir, "local-history");
    const skillsDir = join(maestroDir, "skills");
    const bootstrapDir = join(maestroDir, "bootstrap");
    const configPath = join(maestroDir, "config.yaml");

    await assertProjectLocalPathSafe(opts.dir, maestroDir);
    await assertProjectLocalPathSafe(opts.dir, configPath);

    await ensureDirIfMissing(maestroDir, created);
    await ensureDirIfMissing(tasksDir, created);
    await ensureDirIfMissing(continuationDir, created);
    await ensureDirIfMissing(activeContinuationDir, created);
    await ensureDirIfMissing(completedContinuationDir, created);
    await ensureDirIfMissing(taskLocalHistoryDir, created);
    await ensureDirIfMissing(skillsDir, created);
    await ensureDirIfMissing(bootstrapDir, created);

    if (!(await config.exists("project", opts.dir))) {
      await config.write("project", opts.dir, DEFAULT_CONFIG);
      created.push(configPath);
    } else if (opts.confirmReplace && await opts.confirmReplace(configPath)) {
      await config.write("project", opts.dir, DEFAULT_CONFIG);
      created.push(configPath);
    } else {
      skipped.push(configPath);
    }

    const bootstrapFiles = await collectProjectBootstrapFiles(opts.dir);

    for (const template of bootstrapFiles) {
      const target = join(opts.dir, template.path);
      await assertProjectLocalPathSafe(opts.dir, target);
      await ensureDir(dirname(target));

      const existing = await readText(target);
      if (existing !== undefined) {
        if (!shouldAutoMigrateLegacyTemplate(template.path, existing, template.content)) {
          if (!opts.confirmReplace || !(await opts.confirmReplace(target))) {
            skipped.push(target);
            continue;
          }
        }
      }

      await writeText(target, template.content);
      if (template.executable && process.platform !== "win32") {
        await chmod(target, 0o755);
      }
      created.push(target);
    }

    const gitignoreUpdated = await ensureRuntimeGitignore(opts.dir);
    if (gitignoreUpdated) {
      created.push(join(opts.dir, ".gitignore"));
    } else if (await readText(join(opts.dir, ".gitignore")) !== undefined) {
      skipped.push(join(opts.dir, ".gitignore"));
    }

    const principlesDir = join(opts.dir, "docs", "principles");
    await ensureDir(principlesDir);
    for (const principle of DEFAULT_PRINCIPLES) {
      const principleFile = join(principlesDir, `${principle.slug}.md`);
      if (await readText(principleFile) === undefined) {
        await writeText(principleFile, principle.content);
        created.push(principleFile);
      } else {
        skipped.push(principleFile);
      }
    }

    await syncProjectAgentBuiltInSkills(opts.dir, created);
  }

  return {
    created,
    skipped,
    scope,
    bootstrapGenerated: scope === "project",
  };
}

async function collectProjectBootstrapFiles(rootDir: string): Promise<BootstrapTemplateFile[]> {
  const files = new Map<string, BootstrapTemplateFile>(
    PROJECT_BOOTSTRAP_TEMPLATES.map((template) => [template.path, template]),
  );

  await overlayLegacyFile(files, rootDir, ".factory/AGENTS.md", ".maestro/AGENTS.md");
  await overlayLegacyFile(files, rootDir, ".factory/init.sh", ".maestro/bootstrap/init.sh", true);
  await overlayLegacyFile(files, rootDir, ".factory/services.yaml", ".maestro/bootstrap/services.yaml");
  await overlayLegacyTree(files, rootDir, ".factory/library", ".maestro/bootstrap/library");
  await overlayLegacyTree(files, rootDir, ".factory/validation", ".maestro/bootstrap/validation");
  await overlayLegacyTree(files, rootDir, ".factory/skills", ".maestro/skills");

  return [...files.values()];
}

async function overlayLegacyFile(
  files: Map<string, BootstrapTemplateFile>,
  rootDir: string,
  sourcePath: string,
  targetPath: string,
  executable = false,
): Promise<void> {
  const absoluteSource = join(rootDir, sourcePath);
  const content = await readText(absoluteSource);
  if (content === undefined) {
    return;
  }

  files.set(targetPath, { path: targetPath, content, executable });
}

async function overlayLegacyTree(
  files: Map<string, BootstrapTemplateFile>,
  rootDir: string,
  sourceDir: string,
  targetDir: string,
): Promise<void> {
  const absoluteSourceDir = join(rootDir, sourceDir);
  if (!(await dirExists(absoluteSourceDir))) {
    return;
  }

  for (const sourcePath of await listFilesRecursive(absoluteSourceDir)) {
    const relativePath = relative(absoluteSourceDir, sourcePath);
    const content = await readText(sourcePath);
    if (content === undefined) {
      continue;
    }

    const stat = await lstat(sourcePath);
    // Template map keys are canonical POSIX paths (`.maestro/bootstrap/...`)
    // everywhere else; use posix.join so Windows backslash segments don't
    // split the map into two disjoint entries that silently overwrite each
    // other later during the write phase.
    const key = posix.join(targetDir, relativePath.split(sep).join("/"));
    files.set(key, {
      path: key,
      content,
      executable: isExecutable(stat.mode, relativePath),
    });
  }
}

function isExecutable(mode: number, relativePath: string): boolean {
  if (process.platform === "win32") {
    const ext = relativePath.slice(relativePath.lastIndexOf(".")).toLowerCase();
    return ext === ".exe" || ext === ".cmd" || ext === ".bat" || ext === ".ps1";
  }
  return Boolean(mode & 0o111);
}


function shouldAutoMigrateLegacyTemplate(
  relativePath: string,
  existingContent: string,
  nextContent: string,
): boolean {
  const defaultTemplate = PROJECT_BOOTSTRAP_TEMPLATES.find((template) => template.path === relativePath);
  if (!defaultTemplate) {
    return false;
  }

  return existingContent === defaultTemplate.content && nextContent !== defaultTemplate.content;
}

async function ensureRuntimeGitignore(rootDir: string): Promise<boolean> {
  const gitignorePath = join(rootDir, ".gitignore");
  await assertProjectLocalPathSafe(rootDir, gitignorePath);

  const existing = await readText(gitignorePath) ?? "";
  const lines = new Set(existing.split(/\r?\n/));
  const missingLines = RUNTIME_GITIGNORE_LINES.filter((line) => !lines.has(line));

  if (missingLines.length === 0) {
    return false;
  }

  const prefix = existing.length === 0 ? "" : existing.endsWith("\n") ? "\n" : "\n\n";
  const comment = lines.has(RUNTIME_GITIGNORE_COMMENT) ? "" : `${RUNTIME_GITIGNORE_COMMENT}\n`;
  await writeText(gitignorePath, `${existing}${prefix}${comment}${missingLines.join("\n")}\n`);
  return true;
}

async function ensureDirIfMissing(dir: string, created: string[]): Promise<void> {
  if (!(await dirExists(dir))) {
    await ensureDir(dir);
    created.push(dir);
    return;
  }

  await ensureDir(dir);
}

async function syncProjectAgentBuiltInSkills(rootDir: string, created: string[]): Promise<void> {
  for (const segments of MANAGED_AGENT_SKILL_ROOTS) {
    const skillRoot = join(rootDir, ...segments);
    await assertProjectLocalPathSafe(rootDir, skillRoot);
    await ensureDirIfMissing(skillRoot, created);
    await removeStaleManagedSkillDirs(rootDir, skillRoot);

    for (const template of BUILT_IN_SKILL_TEMPLATES) {
      await syncManagedSkillTemplate(rootDir, skillRoot, template, created);
    }
  }
}

async function removeStaleManagedSkillDirs(rootDir: string, skillRoot: string): Promise<void> {
  const shippedSkillDirNames = new Set(
    BUILT_IN_SKILL_TEMPLATES.map((template) => resolveSkillDirectoryName(template.name)),
  );
  const entries = (await readdir(skillRoot, { withFileTypes: true }))
    .sort((left, right) => left.name.localeCompare(right.name));

  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }

    if (!isManagedSkillDirectoryName(entry.name) || shippedSkillDirNames.has(entry.name)) {
      continue;
    }

    const staleDir = join(skillRoot, entry.name);
    await assertProjectLocalPathSafe(rootDir, staleDir);
    await rm(staleDir, { recursive: true, force: true });
  }
}

async function syncManagedSkillTemplate(
  rootDir: string,
  skillRoot: string,
  template: BuiltInSkillTemplate,
  created: string[],
): Promise<void> {
  const skillDir = join(skillRoot, resolveSkillDirectoryName(template.name));
  await assertProjectLocalPathSafe(rootDir, skillDir);

  if (await skillDirMatchesTemplate(skillDir, template)) {
    return;
  }

  await rm(skillDir, { recursive: true, force: true });
  await ensureDir(skillDir);

  for (const file of template.files) {
    const target = join(skillDir, file.path);
    await assertProjectLocalPathSafe(rootDir, target);
    await ensureDir(dirname(target));
    await writeText(target, file.content);
    created.push(target);
  }
}

async function skillDirMatchesTemplate(skillDir: string, template: BuiltInSkillTemplate): Promise<boolean> {
  if (!(await dirExists(skillDir))) {
    return false;
  }

  const expectedFiles = new Map(template.files.map((file) => [file.path, file.content]));
  const actualFiles = await listFilesRecursive(skillDir);
  if (actualFiles.length !== template.files.length) {
    return false;
  }

  for (const file of actualFiles) {
    // Template file paths are canonical POSIX; on Windows `relative` returns
    // backslash segments, which would miss the expectedFiles map entry and
    // drive a spurious rewrite each idempotent init run.
    const relativePath = relative(skillDir, file).split(sep).join("/");
    const expectedContent = expectedFiles.get(relativePath);
    if (expectedContent === undefined) {
      return false;
    }

    if (await readText(file) !== expectedContent) {
      return false;
    }
  }

  return true;
}
async function assertProjectLocalPathSafe(
  rootDir: string,
  target: string,
): Promise<void> {
  await assertNonSymlinkRoot(rootDir);
  const projectRoot = resolve(rootDir);
  const resolvedTarget = resolve(target);
  const rel = relative(projectRoot, resolvedTarget);

  if (rel === ".." || rel.startsWith(`..${sep}`) || rel === "") {
    throw new Error(`Refusing to initialize outside project root: ${target}`);
  }

  const segments = rel.split(sep).filter(Boolean);
  let current = projectRoot;

  for (const segment of segments) {
    current = join(current, segment);
    try {
      const entry = await lstat(current);
      if (entry.isSymbolicLink()) {
        throw new Error(`Refusing to initialize through symlinked path: ${current}`);
      }
    } catch (err: unknown) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") {
        continue;
      }
      throw err;
    }
  }
}

async function assertNonSymlinkRoot(rootDir: string): Promise<void> {
  try {
    const rootEntry = await lstat(rootDir);
    if (rootEntry.isSymbolicLink()) {
      throw new Error(`Refusing to initialize through symlinked project root: ${rootDir}`);
    }
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") {
      return;
    }
    throw err;
  }
}
