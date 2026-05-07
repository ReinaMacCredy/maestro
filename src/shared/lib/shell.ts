import { spawn } from "node:child_process";
import { closeSync, openSync } from "node:fs";
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

// Uses Bun.spawnSync because repeated async pipe reads leak memory under Bun.
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
    readonly env?: Record<string, string>;
  },
): Promise<LoggedCommandResult> {
  await ensureDir(dirname(opts.logPath));

  const logFd = openSync(opts.logPath, "a");
  let closedFd = false;
  const closeLogFd = (): void => {
    if (closedFd) return;
    closeSync(logFd);
    closedFd = true;
  };

  let child: ReturnType<typeof spawn>;
  try {
    child = spawn(argv[0]!, argv.slice(1), {
      cwd: opts.cwd,
      env: opts.env ? { ...process.env, ...opts.env } : process.env,
      detached: !opts.wait,
      stdio: ["ignore", logFd, logFd],
    });
  } catch (error) {
    closeLogFd();
    throw error;
  }

  await new Promise<void>((resolve, reject) => {
    child.once("spawn", () => resolve());
    child.once("error", (error) => {
      closeLogFd();
      reject(error);
    });
  });

  closeLogFd();

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

  return {
    command: argv,
    pid: child.pid ?? undefined,
    exitCode,
  };
}
