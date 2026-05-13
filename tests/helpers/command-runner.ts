export interface CommandResult {
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}

export interface RunCommandOptions {
  readonly env?: Record<string, string>;
  readonly stdin?: string | Blob | ReadableStream;
}

export async function runCommand(
  command: readonly string[],
  cwd: string = process.cwd(),
  options: RunCommandOptions = {},
): Promise<CommandResult> {
  const proc = Bun.spawn([...command], {
    stdout: "pipe",
    stderr: "pipe",
    stdin: options.stdin === undefined
      ? "inherit"
      : typeof options.stdin === "string"
        ? "pipe"
        : options.stdin,
    cwd,
    env: options.env ? { ...process.env, ...options.env } : process.env,
  });

  if (typeof options.stdin === "string" && proc.stdin && typeof proc.stdin !== "number") {
    proc.stdin.write(options.stdin);
    await proc.stdin.end();
  }

  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);

  return {
    stdout: stdout.trim(),
    stderr: stderr.trim(),
    exitCode: await proc.exited,
  };
}

export async function initGitRepo(cwd: string): Promise<void> {
  const result = await runCommand(["git", "init", "-b", "main"], cwd);
  if (result.exitCode !== 0) {
    throw new Error(`Failed to initialize git repo in ${cwd}: ${result.stderr || result.stdout}`);
  }
}
