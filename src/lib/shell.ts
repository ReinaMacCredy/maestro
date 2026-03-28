import { MaestroError } from "../domain/errors.js";

export interface ShellResult {
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}

/**
 * Execute a shell command and capture output.
 * Uses Bun.spawn for lightweight process execution.
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

export async function exec(
  cmd: string,
  opts: { cwd?: string; timeout?: number } = {},
): Promise<ShellResult> {
  return execSpawn(["sh", "-c", cmd], opts);
}

async function execSpawn(
  argv: string[],
  opts: { cwd?: string; timeout?: number },
): Promise<ShellResult> {
  let proc;
  try {
    proc = Bun.spawn(argv, {
    cwd: opts.cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  } catch {
    return { stdout: "", stderr: `Command not found: ${argv[0]}`, exitCode: 127 };
  }

  const timeoutMs = opts.timeout ?? 30_000;
  const timer = setTimeout(() => proc.kill(), timeoutMs);

  try {
    const [stdout, stderr] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
    ]);
    const exitCode = await proc.exited;

    return {
      stdout: stdout.trim(),
      stderr: stderr.trim(),
      exitCode,
    };
  } finally {
    clearTimeout(timer);
  }
}
