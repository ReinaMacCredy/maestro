import { chmod, lstat, readdir, rename, rm } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join, relative, resolve, sep } from "node:path";
import {
  dirExists,
  ensureDir,
  fileExists,
  listFilesRecursive,
  pathExists,
  readText,
  writeText,
} from "@/shared/lib/fs.js";
import {
  isManagedSkillDirectoryName,
  resolveSkillDirectoryName,
} from "@/shared/lib/skill-path.js";
import type { ConfigPort } from "@/infra/ports/config.port.js";
import { DEFAULT_CONFIG } from "@/infra/domain/config-types.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import {
  PROJECT_BOOTSTRAP_TEMPLATES,
  type BootstrapTemplateFile,
} from "@/infra/domain/bootstrap-templates.js";
import {
  BUILT_IN_SKILL_TEMPLATES,
  type BuiltInSkillTemplate,
} from "@/infra/domain/built-in-skill-templates.js";
import { DEFAULT_PRINCIPLES } from "./default-principles.js";

export type SetupStepStatus = "ok" | "skipped" | "changed" | "error";
export type SetupPathAction =
  | "create"
  | "delete"
  | "move"
  | "overwrite"
  | "skip"
  | "would-create"
  | "would-delete"
  | "would-move"
  | "would-overwrite";

export interface SetupPathEntry {
  readonly path: string;
  readonly action: SetupPathAction;
  readonly detail?: string;
}

export interface SetupStepResult {
  readonly id: string;
  readonly label: string;
  readonly status: SetupStepStatus;
  readonly detail?: string;
  readonly paths: readonly SetupPathEntry[];
}

export interface SetupReport {
  readonly ok: boolean;
  readonly scope: "global" | "project";
  readonly dryRun: boolean;
  readonly steps: readonly SetupStepResult[];
  readonly created: readonly string[];
  readonly skipped: readonly string[];
}

export interface RunSetupOptions {
  readonly dir: string;
  readonly global: boolean;
  readonly config: ConfigPort;
  readonly dryRun?: boolean;
  readonly resyncSkills?: boolean;
  readonly resetTemplates?: boolean;
  readonly noGitOk?: boolean;
  readonly confirmReplace?: (path: string) => Promise<boolean>;
}

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
// `.factory/` is owned by Factory.ai (an unrelated agent tool). Removing it
// would corrupt a user's parallel install. v1 maestro never wrote that path,
// so it does not belong in the v1 cleanup set.
const V1_PATHS_REL = [
  ".maestro/memory/corrections",
  ".maestro/memory/learnings",
  ".maestro/.migrated-v2.json",
] as const;

export async function runSetup(opts: RunSetupOptions): Promise<SetupReport> {
  const scope: "global" | "project" = opts.global ? "global" : "project";
  const dryRun = opts.dryRun === true;

  if (scope === "global") {
    const step = await stepWriteGlobalConfig(opts, dryRun);
    return finish(scope, dryRun, [step]);
  }

  if (!opts.noGitOk && !(await dirExists(join(opts.dir, ".git")))) {
    return {
      ok: false,
      scope,
      dryRun,
      steps: [
        {
          id: "guard-git",
          label: "Guard: not a git working tree",
          status: "error",
          detail: "this directory is not a git repository; run `git init` first, or pass --no-git-ok to set up maestro without git",
          paths: [],
        },
      ],
      created: [],
      skipped: [],
    };
  }

  const steps: SetupStepResult[] = [];
  const detectResult = await stepDetectV1(opts.dir);
  steps.push(detectResult);
  steps.push(await stepDeleteV1(detectResult, dryRun));
  const migrateStep = await stepMigratePlansToMissions(opts.dir, dryRun);
  steps.push(migrateStep);
  if (migrateStep.status === "error") {
    return finish(scope, dryRun, steps);
  }
  steps.push(await stepMigrateTaskFields(opts.dir, dryRun));
  steps.push(await stepBootstrapDirs(opts.dir, dryRun));
  steps.push(await stepWriteProjectConfig(opts, dryRun));
  steps.push(await stepDropTemplates(opts, dryRun));
  steps.push(await stepSeedPrinciples(opts.dir, dryRun));
  steps.push(await stepSyncSkills(opts, dryRun));

  return finish(scope, dryRun, steps);
}

function finish(
  scope: "global" | "project",
  dryRun: boolean,
  steps: readonly SetupStepResult[],
): SetupReport {
  const created: string[] = [];
  const skipped: string[] = [];
  for (const step of steps) {
    for (const entry of step.paths) {
      if (entry.action === "create" || entry.action === "move" || entry.action === "delete") {
        created.push(entry.path);
      } else if (entry.action === "skip") {
        skipped.push(entry.path);
      }
    }
  }
  const ok = steps.every((s) => s.status !== "error");
  return { ok, scope, dryRun, steps, created, skipped };
}

async function stepDetectV1(dir: string): Promise<SetupStepResult> {
  const found: SetupPathEntry[] = [];
  for (const rel of V1_PATHS_REL) {
    const abs = join(dir, rel);
    if (await pathExists(abs)) {
      found.push({ path: abs, action: "skip", detail: "v1 artifact present" });
    }
  }
  return {
    id: "detect-v1",
    label: "Detect v1 leftovers",
    status: found.length === 0 ? "ok" : "changed",
    detail: found.length === 0 ? "no v1 paths found" : `${found.length} v1 path(s) detected`,
    paths: found,
  };
}

async function stepDeleteV1(
  detectResult: SetupStepResult,
  dryRun: boolean,
): Promise<SetupStepResult> {
  if (detectResult.paths.length === 0) {
    return {
      id: "delete-v1",
      label: "Delete v1 leftovers",
      status: "ok",
      detail: "no v1 paths found by detect-v1",
      paths: [],
    };
  }

  const paths: SetupPathEntry[] = [];
  for (const detected of detectResult.paths) {
    const abs = detected.path;
    if (!(await pathExists(abs))) continue;
    if (dryRun) {
      paths.push({ path: abs, action: "would-delete" });
      continue;
    }
    await rm(abs, { recursive: true, force: true });
    paths.push({ path: abs, action: "delete" });
  }
  return {
    id: "delete-v1",
    label: "Delete v1 leftovers",
    status: paths.length === 0 ? "ok" : "changed",
    paths,
  };
}

async function stepMigratePlansToMissions(
  dir: string,
  dryRun: boolean,
): Promise<SetupStepResult> {
  const plansDir = join(dir, ".maestro", "plans");
  const missionsDir = join(dir, ".maestro", "missions");
  await assertProjectLocalPathSafe(dir, plansDir);
  await assertProjectLocalPathSafe(dir, missionsDir);

  if (!(await dirExists(plansDir))) {
    return {
      id: "migrate-plans-to-missions",
      label: "Migrate .maestro/plans/ -> .maestro/missions/",
      status: "ok",
      detail: "no .maestro/plans/ to migrate",
      paths: [],
    };
  }

  const existingMissions = await listFilesIfDir(missionsDir);
  if (existingMissions.some((p) => /\.jsonl$/.test(p))) {
    return {
      id: "migrate-plans-to-missions",
      label: "Migrate .maestro/plans/ -> .maestro/missions/",
      status: "error",
      detail:
        ".maestro/missions/ already has v2 data; resolve manually before re-running setup",
      paths: [{ path: missionsDir, action: "skip" }],
    };
  }

  const paths: SetupPathEntry[] = [];
  if (dryRun) {
    paths.push({ path: missionsDir, action: "would-create" });
    paths.push({ path: plansDir, action: "would-delete" });
    return {
      id: "migrate-plans-to-missions",
      label: "Migrate .maestro/plans/ -> .maestro/missions/",
      status: "changed",
      paths,
    };
  }

  await ensureDir(missionsDir);
  paths.push({ path: missionsDir, action: "create" });

  const entries = await readdir(plansDir, { withFileTypes: true });
  for (const entry of entries) {
    const fromPath = join(plansDir, entry.name);
    const toName = renamePlansFile(entry.name);
    const toPath = join(missionsDir, toName);
    // assertProjectLocalPathSafe lstats every segment, which refuses symlinked
    // entries planted in .maestro/plans/ before they get renamed across roots.
    await assertProjectLocalPathSafe(dir, fromPath);
    await assertProjectLocalPathSafe(dir, toPath);
    await rename(fromPath, toPath);
    if (entry.isFile() && entry.name.endsWith(".jsonl")) {
      const rewritten = await rewriteMissionJsonlContent(toPath);
      if (rewritten > 0) {
        paths.push({
          path: toPath,
          action: "move",
          detail: `from ${fromPath}; rewrote ${rewritten} row(s)`,
        });
        continue;
      }
    }
    paths.push({ path: toPath, action: "move", detail: `from ${fromPath}` });
  }

  await rm(plansDir, { recursive: true, force: true });
  paths.push({ path: plansDir, action: "delete" });

  return {
    id: "migrate-plans-to-missions",
    label: "Migrate .maestro/plans/ -> .maestro/missions/",
    status: "changed",
    paths,
  };
}

function statusFor(paths: readonly SetupPathEntry[]): SetupStepStatus {
  return paths.some((p) => p.action !== "skip") ? "changed" : "ok";
}

function renamePlansFile(name: string): string {
  if (name === "plans.jsonl") return "missions.jsonl";
  if (name === "plans.v2.jsonl") return "missions.v2.jsonl";
  return name;
}

// v1 plan state "specified" was renamed to "approved" in the 8-state mission
// machine. Rewrite migrated jsonl rows so the mission store doesn't read
// unknown state values.
async function rewriteMissionJsonlContent(file: string): Promise<number> {
  const text = await readText(file);
  if (text === undefined || text.length === 0) return 0;
  let changed = 0;
  const out: string[] = [];
  for (const line of text.split("\n")) {
    if (line.length === 0) {
      out.push(line);
      continue;
    }
    try {
      const row = JSON.parse(line) as Record<string, unknown>;
      if (row.state === "specified") {
        row.state = "approved";
        changed += 1;
      }
      out.push(JSON.stringify(row));
    } catch {
      out.push(line);
    }
  }
  if (changed === 0) return 0;
  const body = out.join("\n");
  await writeText(file, body.endsWith("\n") ? body : `${body}\n`);
  return changed;
}

// v1 tasks carried `plan_id`; v2 renamed it to `mission_id`. Rewrite in place
// so the task store doesn't reject legacy rows on read.
async function stepMigrateTaskFields(
  dir: string,
  dryRun: boolean,
): Promise<SetupStepResult> {
  const tasksFile = join(dir, ".maestro", "tasks", "tasks.jsonl");
  await assertProjectLocalPathSafe(dir, tasksFile);
  const text = await readText(tasksFile);
  if (text === undefined || text.length === 0) {
    return {
      id: "migrate-task-fields",
      label: "Rewrite legacy task fields (plan_id -> mission_id)",
      status: "ok",
      detail: "no .maestro/tasks/tasks.jsonl to migrate",
      paths: [],
    };
  }
  let changed = 0;
  const out: string[] = [];
  for (const line of text.split("\n")) {
    if (line.length === 0) {
      out.push(line);
      continue;
    }
    try {
      const row = JSON.parse(line) as Record<string, unknown>;
      if ("plan_id" in row && !("mission_id" in row)) {
        row.mission_id = row.plan_id;
        delete row.plan_id;
        changed += 1;
      } else if ("plan_id" in row) {
        delete row.plan_id;
        changed += 1;
      }
      out.push(JSON.stringify(row));
    } catch {
      out.push(line);
    }
  }
  if (changed === 0) {
    return {
      id: "migrate-task-fields",
      label: "Rewrite legacy task fields (plan_id -> mission_id)",
      status: "ok",
      detail: "no legacy task fields found",
      paths: [],
    };
  }
  if (dryRun) {
    return {
      id: "migrate-task-fields",
      label: "Rewrite legacy task fields (plan_id -> mission_id)",
      status: "changed",
      detail: `${changed} row(s) need rewrite`,
      paths: [{ path: tasksFile, action: "would-overwrite" }],
    };
  }
  const body = out.join("\n");
  await writeText(tasksFile, body.endsWith("\n") ? body : `${body}\n`);
  return {
    id: "migrate-task-fields",
    label: "Rewrite legacy task fields (plan_id -> mission_id)",
    status: "changed",
    detail: `rewrote ${changed} row(s)`,
    paths: [{ path: tasksFile, action: "overwrite" }],
  };
}

async function stepBootstrapDirs(dir: string, dryRun: boolean): Promise<SetupStepResult> {
  const dirs = [
    join(dir, MAESTRO_DIR),
    join(dir, MAESTRO_DIR, "tasks"),
    join(dir, MAESTRO_DIR, "tasks", "continuations"),
    join(dir, MAESTRO_DIR, "tasks", "continuations", "active"),
    join(dir, MAESTRO_DIR, "tasks", "continuations", "completed"),
    join(dir, MAESTRO_DIR, "tasks", "local-history"),
    join(dir, MAESTRO_DIR, "missions"),
    join(dir, MAESTRO_DIR, "evidence"),
    join(dir, MAESTRO_DIR, "runs"),
    join(dir, MAESTRO_DIR, "skills"),
    join(dir, MAESTRO_DIR, "bootstrap"),
    join(dir, MAESTRO_DIR, "templates", "missions"),
  ];

  const paths: SetupPathEntry[] = [];
  for (const d of dirs) {
    await assertProjectLocalPathSafe(dir, d);
    const exists = await dirExists(d);
    if (exists) continue;
    if (dryRun) {
      paths.push({ path: d, action: "would-create" });
    } else {
      await ensureDir(d);
      paths.push({ path: d, action: "create" });
    }
  }

  const templatesGitkeep = join(dir, MAESTRO_DIR, "templates", "missions", ".gitkeep");
  if (!(await fileExists(templatesGitkeep))) {
    if (dryRun) {
      paths.push({ path: templatesGitkeep, action: "would-create" });
    } else {
      await writeText(templatesGitkeep, "");
      paths.push({ path: templatesGitkeep, action: "create" });
    }
  }

  const gitignoreResult = await ensureRuntimeGitignore(dir, dryRun);
  if (gitignoreResult) paths.push(gitignoreResult);

  return {
    id: "bootstrap-dirs",
    label: "Bootstrap .maestro/ directories",
    status: statusFor(paths),
    paths,
  };
}

async function stepWriteGlobalConfig(
  opts: RunSetupOptions,
  dryRun: boolean,
): Promise<SetupStepResult> {
  const globalDir = join(homedir(), MAESTRO_DIR);
  const configPath = join(globalDir, "config.yaml");
  const paths: SetupPathEntry[] = [];

  if (!(await dirExists(globalDir))) {
    if (dryRun) {
      paths.push({ path: globalDir, action: "would-create" });
    } else {
      await ensureDir(globalDir);
      paths.push({ path: globalDir, action: "create" });
    }
  }

  if (!(await opts.config.exists("global", opts.dir))) {
    if (dryRun) {
      paths.push({ path: configPath, action: "would-create" });
    } else {
      await opts.config.write("global", opts.dir, DEFAULT_CONFIG);
      paths.push({ path: configPath, action: "create" });
    }
  } else {
    paths.push({ path: configPath, action: "skip" });
  }

  return {
    id: "write-config-global",
    label: "Write global config",
    status: statusFor(paths),
    paths,
  };
}

async function stepWriteProjectConfig(
  opts: RunSetupOptions,
  dryRun: boolean,
): Promise<SetupStepResult> {
  const configPath = join(opts.dir, MAESTRO_DIR, "config.yaml");
  await assertProjectLocalPathSafe(opts.dir, configPath);
  const exists = await opts.config.exists("project", opts.dir);
  const paths: SetupPathEntry[] = [];

  if (!exists) {
    if (dryRun) {
      paths.push({ path: configPath, action: "would-create" });
    } else {
      await opts.config.write("project", opts.dir, DEFAULT_CONFIG);
      paths.push({ path: configPath, action: "create" });
    }
  } else if (opts.confirmReplace && (await opts.confirmReplace(configPath))) {
    if (dryRun) {
      paths.push({ path: configPath, action: "would-create" });
    } else {
      await opts.config.write("project", opts.dir, DEFAULT_CONFIG);
      paths.push({ path: configPath, action: "create" });
    }
  } else {
    paths.push({ path: configPath, action: "skip" });
  }

  return {
    id: "write-config-project",
    label: "Write project config",
    status: statusFor(paths),
    paths,
  };
}

async function stepDropTemplates(
  opts: RunSetupOptions,
  dryRun: boolean,
): Promise<SetupStepResult> {
  const paths: SetupPathEntry[] = [];
  for (const template of PROJECT_BOOTSTRAP_TEMPLATES) {
    const target = join(opts.dir, template.path);
    await assertProjectLocalPathSafe(opts.dir, target);

    const existing = await readText(target);
    if (existing !== undefined) {
      const allowAutoMigrate =
        opts.resetTemplates === true &&
        shouldAutoMigrateLegacyTemplate(template.path, existing, template.content);
      const okToReplace =
        allowAutoMigrate ||
        (opts.confirmReplace !== undefined && (await opts.confirmReplace(target)));
      if (!okToReplace) {
        paths.push({ path: target, action: "skip" });
        continue;
      }
    }

    if (dryRun) {
      paths.push({ path: target, action: "would-create" });
      continue;
    }

    await ensureDir(dirname(target));
    await writeText(target, template.content);
    if (template.executable && process.platform !== "win32") {
      await chmod(target, 0o755);
    }
    paths.push({ path: target, action: "create" });
  }

  return {
    id: "drop-templates",
    label: "Drop project bootstrap templates",
    status: statusFor(paths),
    paths,
  };
}

async function stepSeedPrinciples(dir: string, dryRun: boolean): Promise<SetupStepResult> {
  const principlesDir = join(dir, "docs", "principles");
  const paths: SetupPathEntry[] = [];
  if (!(await dirExists(principlesDir))) {
    if (dryRun) {
      paths.push({ path: principlesDir, action: "would-create" });
    } else {
      await ensureDir(principlesDir);
      paths.push({ path: principlesDir, action: "create" });
    }
  }

  for (const principle of DEFAULT_PRINCIPLES) {
    const file = join(principlesDir, `${principle.slug}.md`);
    if (await fileExists(file)) {
      paths.push({ path: file, action: "skip" });
      continue;
    }
    if (dryRun) {
      paths.push({ path: file, action: "would-create" });
    } else {
      await writeText(file, principle.content);
      paths.push({ path: file, action: "create" });
    }
  }

  return {
    id: "seed-principles",
    label: "Seed docs/principles",
    status: statusFor(paths),
    paths,
  };
}

async function stepSyncSkills(
  opts: RunSetupOptions,
  dryRun: boolean,
): Promise<SetupStepResult> {
  const paths: SetupPathEntry[] = [];
  for (const segments of MANAGED_AGENT_SKILL_ROOTS) {
    const skillRoot = join(opts.dir, ...segments);
    await assertProjectLocalPathSafe(opts.dir, skillRoot);

    if (!(await dirExists(skillRoot))) {
      if (dryRun) {
        paths.push({ path: skillRoot, action: "would-create" });
      } else {
        await ensureDir(skillRoot);
        paths.push({ path: skillRoot, action: "create" });
      }
    }

    if (opts.resyncSkills === true) {
      await removeStaleManagedSkillDirs(opts.dir, skillRoot, dryRun, paths);
    }

    for (const template of BUILT_IN_SKILL_TEMPLATES) {
      await syncManagedSkillTemplate(opts.dir, skillRoot, template, dryRun, paths);
    }
  }

  return {
    id: "sync-skills",
    label: "Sync managed agent skills",
    status: statusFor(paths),
    paths,
  };
}

async function removeStaleManagedSkillDirs(
  rootDir: string,
  skillRoot: string,
  dryRun: boolean,
  paths: SetupPathEntry[],
): Promise<void> {
  const shippedSkillDirNames = new Set(
    BUILT_IN_SKILL_TEMPLATES.map((template) => resolveSkillDirectoryName(template.name)),
  );
  let entries;
  try {
    entries = (await readdir(skillRoot, { withFileTypes: true })).sort((a, b) =>
      a.name.localeCompare(b.name),
    );
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return;
    throw err;
  }

  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    if (!isManagedSkillDirectoryName(entry.name) || shippedSkillDirNames.has(entry.name)) continue;
    const staleDir = join(skillRoot, entry.name);
    await assertProjectLocalPathSafe(rootDir, staleDir);
    if (dryRun) {
      paths.push({ path: staleDir, action: "would-delete" });
    } else {
      await rm(staleDir, { recursive: true, force: true });
      paths.push({ path: staleDir, action: "delete" });
    }
  }
}

async function syncManagedSkillTemplate(
  rootDir: string,
  skillRoot: string,
  template: BuiltInSkillTemplate,
  dryRun: boolean,
  paths: SetupPathEntry[],
): Promise<void> {
  const skillDir = join(skillRoot, resolveSkillDirectoryName(template.name));
  await assertProjectLocalPathSafe(rootDir, skillDir);

  if (await skillDirMatchesTemplate(skillDir, template)) {
    return;
  }

  // Replacing an existing skill dir wipes any user edits to shipped files.
  // Surface that explicitly so the report reflects the destructive write.
  const replacing = await dirExists(skillDir);
  const fileAction: SetupPathAction = replacing
    ? dryRun ? "would-overwrite" : "overwrite"
    : dryRun ? "would-create" : "create";

  if (dryRun) {
    if (replacing) {
      paths.push({
        path: skillDir,
        action: "would-overwrite",
        detail: "replaces shipped skill dir (user edits will be lost)",
      });
    }
    for (const file of template.files) {
      paths.push({ path: join(skillDir, file.path), action: fileAction });
    }
    return;
  }

  if (replacing) {
    paths.push({
      path: skillDir,
      action: "overwrite",
      detail: "replaced shipped skill dir (user edits discarded)",
    });
  }
  await rm(skillDir, { recursive: true, force: true });
  await ensureDir(skillDir);
  for (const file of template.files) {
    const target = join(skillDir, file.path);
    await assertProjectLocalPathSafe(rootDir, target);
    await ensureDir(dirname(target));
    await writeText(target, file.content);
    paths.push({ path: target, action: fileAction });
  }
}

async function skillDirMatchesTemplate(
  skillDir: string,
  template: BuiltInSkillTemplate,
): Promise<boolean> {
  if (!(await dirExists(skillDir))) return false;

  const expectedFiles = new Map(template.files.map((file) => [file.path, file.content]));
  const actualFiles = await listFilesRecursive(skillDir);
  if (actualFiles.length !== template.files.length) return false;

  const matches = await Promise.all(
    actualFiles.map(async (file) => {
      const relativePath = relative(skillDir, file).split(sep).join("/");
      const expected = expectedFiles.get(relativePath);
      if (expected === undefined) return false;
      return (await readText(file)) === expected;
    }),
  );
  return matches.every(Boolean);
}

async function ensureRuntimeGitignore(
  rootDir: string,
  dryRun: boolean,
): Promise<SetupPathEntry | undefined> {
  const gitignorePath = join(rootDir, ".gitignore");
  await assertProjectLocalPathSafe(rootDir, gitignorePath);

  const existing = (await readText(gitignorePath)) ?? "";
  const lines = new Set(existing.split(/\r?\n/));
  const missingLines = RUNTIME_GITIGNORE_LINES.filter((line) => !lines.has(line));
  if (missingLines.length === 0) {
    if (existing.length > 0) return { path: gitignorePath, action: "skip" };
    return undefined;
  }

  if (dryRun) return { path: gitignorePath, action: "would-create" };

  const prefix = existing.length === 0 ? "" : existing.endsWith("\n") ? "\n" : "\n\n";
  const comment = lines.has(RUNTIME_GITIGNORE_COMMENT) ? "" : `${RUNTIME_GITIGNORE_COMMENT}\n`;
  await writeText(gitignorePath, `${existing}${prefix}${comment}${missingLines.join("\n")}\n`);
  return { path: gitignorePath, action: "create" };
}

function shouldAutoMigrateLegacyTemplate(
  relativePath: string,
  existingContent: string,
  nextContent: string,
): boolean {
  const defaultTemplate = PROJECT_BOOTSTRAP_TEMPLATES.find((t) => t.path === relativePath);
  if (!defaultTemplate) return false;
  return existingContent === defaultTemplate.content && nextContent !== defaultTemplate.content;
}

async function listFilesIfDir(dir: string): Promise<readonly string[]> {
  try {
    return await readdir(dir);
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw err;
  }
}

async function assertProjectLocalPathSafe(rootDir: string, target: string): Promise<void> {
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
      if ((err as NodeJS.ErrnoException).code === "ENOENT") continue;
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
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return;
    throw err;
  }
}

