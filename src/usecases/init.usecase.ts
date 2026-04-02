import type { ConfigPort } from "../ports/config.port.js";
import { DEFAULT_CONFIG, MAESTRO_DIR } from "../domain/defaults.js";
import { PROJECT_BOOTSTRAP_TEMPLATES } from "../domain/bootstrap-templates.js";
import { dirExists, ensureDir, writeText } from "../lib/fs.js";
import { homedir } from "node:os";
import { dirname, join, relative, resolve, sep } from "node:path";
import { chmod, lstat } from "node:fs/promises";

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
    const handoffsDir = join(maestroDir, "handoffs");
    const skillsDir = join(maestroDir, "skills");
    const bootstrapDir = join(maestroDir, "bootstrap");
    const configPath = join(maestroDir, "config.yaml");

    await assertProjectLocalPathSafe(opts.dir, maestroDir);
    await assertProjectLocalPathSafe(opts.dir, configPath);

    await ensureDirIfMissing(maestroDir, created);
    await ensureDirIfMissing(handoffsDir, created);
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

    for (const template of PROJECT_BOOTSTRAP_TEMPLATES) {
      const target = join(opts.dir, template.path);
      await assertProjectLocalPathSafe(opts.dir, target);
      await ensureDir(dirname(target));

      if (await Bun.file(target).exists()) {
        if (!opts.confirmReplace || !(await opts.confirmReplace(target))) {
          skipped.push(target);
          continue;
        }
      }

      await writeText(target, template.content);
      if (template.executable) {
        await chmod(target, 0o755);
      }
      created.push(target);
    }
  }

  return {
    created,
    skipped,
    scope,
    bootstrapGenerated: scope === "project",
  };
}

async function ensureDirIfMissing(dir: string, created: string[]): Promise<void> {
  if (!(await dirExists(dir))) {
    await ensureDir(dir);
    created.push(dir);
    return;
  }

  await ensureDir(dir);
}

async function assertProjectLocalPathSafe(
  rootDir: string,
  target: string,
): Promise<void> {
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
