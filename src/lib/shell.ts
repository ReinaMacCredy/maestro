export interface ShellResult {
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}

/**
 * Execute a shell command and capture output.
 * Uses Bun.spawn for lightweight process execution.
 */
export async function exec(
  cmd: string,
  opts: { cwd?: string; timeout?: number } = {},
): Promise<ShellResult> {
  const proc = Bun.spawn(["sh", "-c", cmd], {
    cwd: opts.cwd,
    stdout: "pipe",
    stderr: "pipe",
  });

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
