import { spawn } from "node:child_process";
import { createWriteStream } from "node:fs";
import { dirname } from "node:path";
import { MaestroError } from "@/shared/errors.js";
import { ensureDir } from "./fs.js";

export interface ShellResult {
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}

export interface LoggedCommandResult {
  readonly command: readonly string[];
  readonly pid?: number;
  readonly exitCode?: number;
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

export async function runLoggedCommand(
  argv: string[],
  opts: {
    readonly cwd?: string;
    readonly logPath: string;
    readonly wait: boolean;
  },
): Promise<LoggedCommandResult> {
  await ensureDir(dirname(opts.logPath));

  const logStream = createWriteStream(opts.logPath, { flags: "a" });
  const child = spawn(argv[0]!, argv.slice(1), {
    cwd: opts.cwd,
    detached: !opts.wait,
    stdio: ["ignore", "pipe", "pipe"],
  });

  child.stdout?.pipe(logStream, { end: false });
  child.stderr?.pipe(logStream, { end: false });

  await new Promise<void>((resolve, reject) => {
    child.once("spawn", () => resolve());
    child.once("error", (error) => reject(error));
  });

  if (!opts.wait) {
    child.unref();
    return {
      command: argv,
      pid: child.pid ?? undefined,
    };
  }

  const exitCode = await new Promise<number>((resolve, reject) => {
    child.once("close", (code, signal) => resolve(code ?? (signal ? 1 : 0)));
    child.once("error", (error) => reject(error));
  });

  await new Promise<void>((resolve, reject) => {
    logStream.end((error) => error ? reject(error) : resolve());
  });

  return {
    command: argv,
    pid: child.pid ?? undefined,
    exitCode,
  };
}
