/**
 * Self-update usecase: detect install method, git pull, rebuild.
 */

import * as fs from 'fs';
import * as path from 'path';
import { MaestroError } from '../../core/errors.ts';

export interface ExecResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

export interface SelfUpdateServices {
  exec: (cmd: string[], opts: { cwd: string }) => Promise<ExecResult>;
}

export type InstallMethod = 'symlink-to-repo' | 'in-repo' | 'standalone' | 'unknown';

export interface DetectResult {
  method: InstallMethod;
  repoPath: string | null;
  binaryPath: string;
}

export interface SelfUpdateResult {
  updated: boolean;
  beforeSha: string;
  afterSha: string;
  repoPath: string;
  binaryPath: string;
}

/**
 * Walk up from realPath looking for a directory with both .git/ and
 * a package.json containing "maestro-cli".
 */
function findRepoRoot(startPath: string): string | null {
  let dir = path.dirname(startPath);
  const root = path.parse(dir).root;

  while (dir !== root) {
    const pkgPath = path.join(dir, 'package.json');
    const gitPath = path.join(dir, '.git');
    if (fs.existsSync(gitPath) && fs.existsSync(pkgPath)) {
      try {
        const content = fs.readFileSync(pkgPath, 'utf-8');
        if (content.includes('"maestro-cli"')) {
          return dir;
        }
      } catch {
        // unreadable package.json -- keep walking
      }
    }
    dir = path.dirname(dir);
  }
  return null;
}

export function detectInstall(execPath: string): DetectResult {
  let realPath: string;
  try {
    realPath = fs.realpathSync(execPath);
  } catch {
    return { method: 'unknown', repoPath: null, binaryPath: execPath };
  }

  const repoPath = findRepoRoot(realPath);

  if (!repoPath) {
    return { method: 'standalone', repoPath: null, binaryPath: realPath };
  }

  const isSymlink = realPath !== execPath;
  return {
    method: isSymlink ? 'symlink-to-repo' : 'in-repo',
    repoPath,
    binaryPath: realPath,
  };
}

export async function selfUpdate(
  services: SelfUpdateServices,
  params: { repoPath: string; binaryPath: string },
): Promise<SelfUpdateResult> {
  const { exec } = services;
  const { repoPath, binaryPath } = params;

  // Pre-checks: branch, bun, and current SHA are independent reads
  const [branchResult, bunCheck, beforeResult] = await Promise.all([
    exec(['git', 'rev-parse', '--abbrev-ref', 'HEAD'], { cwd: repoPath }),
    exec(['which', 'bun'], { cwd: repoPath }),
    exec(['git', 'rev-parse', 'HEAD'], { cwd: repoPath }),
  ]);

  if (branchResult.exitCode !== 0) {
    throw new MaestroError('failed to detect git branch', [
      `Run: cd ${repoPath} && git status`,
    ]);
  }
  const branch = branchResult.stdout.trim();
  if (branch !== 'main') {
    throw new MaestroError(
      `repo is on branch "${branch}", expected "main"`,
      [`Switch to main: cd ${repoPath} && git checkout main`],
    );
  }

  if (bunCheck.exitCode !== 0) {
    throw new MaestroError(
      'bun not found in PATH',
      ['Install bun: https://bun.sh/docs/installation'],
    );
  }

  if (beforeResult.exitCode !== 0) {
    throw new MaestroError('failed to read current commit SHA');
  }
  const beforeSha = beforeResult.stdout.trim();

  const pullResult = await exec(['git', 'pull', '--ff-only'], { cwd: repoPath });
  if (pullResult.exitCode !== 0) {
    throw new MaestroError(
      `git pull --ff-only failed: ${pullResult.stderr.trim()}`,
      [`Resolve manually: cd ${repoPath} && git status`],
    );
  }

  const afterResult = await exec(['git', 'rev-parse', 'HEAD'], { cwd: repoPath });
  if (afterResult.exitCode !== 0) {
    throw new MaestroError('failed to read new commit SHA');
  }
  const afterSha = afterResult.stdout.trim();

  if (beforeSha === afterSha) {
    return { updated: false, beforeSha, afterSha, repoPath, binaryPath };
  }

  const buildResult = await exec(['bun', 'run', 'build'], { cwd: repoPath });
  if (buildResult.exitCode !== 0) {
    throw new MaestroError(
      'build failed after repo update',
      [`Repo updated but binary is stale. Run: cd ${repoPath} && bun run build`],
    );
  }

  return { updated: true, beforeSha, afterSha, repoPath, binaryPath };
}
