/**
 * maestro self-update -- update maestro to latest version.
 */

import { defineCommand } from 'citty';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError, MaestroError } from '../../../../domain/errors.ts';
import {
  detectInstall,
  selfUpdate,
  type ExecResult,
  type SelfUpdateResult,
} from '../../../../app/usecases/self-update.ts';

async function execCommand(cmd: string[], opts: { cwd: string }): Promise<ExecResult> {
  const proc = Bun.spawn(cmd, {
    cwd: opts.cwd,
    stdout: 'pipe',
    stderr: 'pipe',
  });
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  const exitCode = await proc.exited;
  return { exitCode, stdout, stderr };
}

function formatResult(result: SelfUpdateResult): string {
  if (!result.updated) {
    return `[ok] maestro is already up-to-date (${result.beforeSha.slice(0, 7)})`;
  }
  return [
    `[ok] maestro updated (${result.beforeSha.slice(0, 7)} --> ${result.afterSha.slice(0, 7)})`,
    `  repo: ${result.repoPath}`,
    `  binary rebuilt at ${result.binaryPath}`,
  ].join('\n');
}

export async function runSelfUpdate() {
  try {
    const install = detectInstall(process.execPath);
    if (!install.repoPath) {
      throw new MaestroError(
        `unsupported install method "${install.method}"`,
        [
          'maestro must be installed as a symlink to the repo build output.',
          'Fix: ln -sf <repo>/dist/maestro ~/.local/bin/maestro',
        ],
      );
    }
    const result = await selfUpdate({ exec: execCommand }, { repoPath: install.repoPath, binaryPath: install.binaryPath });
    output(result, formatResult);
  } catch (err) {
    handleCommandError('self-update', err);
  }
}

export default defineCommand({
  meta: { name: 'self-update', description: 'Update maestro to latest version\n\nExamples:\n  maestro self-update\n  maestro self-update --json' },
  args: {},
  run: runSelfUpdate,
});
