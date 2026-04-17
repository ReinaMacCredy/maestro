import { MaestroError } from "@/shared/errors.js";

export interface ShellResult {
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}

/**
 * Execute a shell command and capture output.
 * Uses Bun.spawnSync because repeated async pipe reads leak memory under Bun.
 */
export async function execArgv(
  argv: string[],
  opts: { cwd?: string; timeout?: number } = {},
): Promise<ShellResult> {
  return execSpawn(argv, opts);
}

export async function execOrThrow(
  argv: string[],
  name: string,
  opts?: { cwd?: string },
): Promise<ShellResult> {
  const result = await execArgv(argv, opts);
  if (result.exitCode !== 0) {
    throw new MaestroError(`${name} failed: ${result.stderr}`, [
      `Command: ${argv.join(" ")}`,
    ]);
  }
  return result;
}

async function execSpawn(
  argv: string[],
  opts: { cwd?: string; timeout?: number },
): Promise<ShellResult> {
  let proc;
  try {
    proc = Bun.spawnSync(argv, {
      cwd: opts.cwd,
      stdout: "pipe",
      stderr: "pipe",
      timeout: opts.timeout ?? 30_000,
    });
  } catch {
    return { stdout: "", stderr: `Command not found: ${argv[0]}`, exitCode: 127 };
  }

  if (proc.exitedDueToTimeout) {
    const timeoutMs = opts.timeout ?? 30_000;
    return {
      stdout: "",
      stderr: `Command timed out after ${timeoutMs}ms`,
      exitCode: 124,
    };
  }

  return {
    stdout: proc.stdout.toString().trim(),
    stderr: proc.stderr.toString().trim(),
    exitCode: proc.exitCode ?? 1,
  };
}
