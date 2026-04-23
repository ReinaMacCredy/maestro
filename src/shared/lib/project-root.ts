import { existsSync, readFileSync, realpathSync, statSync } from "node:fs";
import { basename, dirname, join, parse, resolve } from "node:path";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";

const GIT_DIR = ".git";
const GITDIR_LINE_PATTERN = /^gitdir:\s*(.+)$/m;
const COMMON_GIT_DIR_FILE = "commondir";

export function resolveMaestroProjectRoot(startDir: string): string {
  let current = safeRealpath(startDir);
  const root = parse(current).root;
  let gitFallback: string | undefined;

  while (true) {
    if (existsSync(join(current, MAESTRO_DIR))) {
      return current;
    }
    const gitPath = join(current, GIT_DIR);
    if (existsSync(gitPath)) {
      gitFallback ??= current;
      const worktreeRoot = resolveMaestroRootFromGitFile(gitPath);
      if (worktreeRoot) return worktreeRoot;
    }
    if (current === root) {
      return gitFallback ?? startDir;
    }
    current = dirname(current);
  }
}

function resolveMaestroRootFromGitFile(gitPath: string): string | undefined {
  if (!isFile(gitPath)) return undefined;
  try {
    const match = GITDIR_LINE_PATTERN.exec(readFileSync(gitPath, "utf8"));
    const rawGitDir = match?.[1]?.trim();
    if (!rawGitDir) return undefined;

    const worktreeGitDir = safeRealpath(resolve(dirname(gitPath), rawGitDir));
    const commonGitDir = readCommonGitDir(worktreeGitDir);
    if (basename(commonGitDir) !== GIT_DIR) return undefined;

    const candidateRoot = dirname(commonGitDir);
    return existsSync(join(candidateRoot, MAESTRO_DIR)) ? candidateRoot : undefined;
  } catch {
    return undefined;
  }
}

function readCommonGitDir(worktreeGitDir: string): string {
  try {
    const rawCommonDir = readFileSync(join(worktreeGitDir, COMMON_GIT_DIR_FILE), "utf8").trim();
    if (rawCommonDir.length > 0) {
      return safeRealpath(resolve(worktreeGitDir, rawCommonDir));
    }
  } catch {
    // Non-worktree git files do not have commondir; use the gitdir itself.
  }
  return worktreeGitDir;
}

function isFile(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

function safeRealpath(value: string): string {
  try {
    return realpathSync(value);
  } catch {
    return value;
  }
}
