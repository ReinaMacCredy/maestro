import { open, stat } from "node:fs/promises";
import { setTimeout as sleep } from "node:timers/promises";
import { MaestroError } from "@/shared/errors.js";
import { readText, removeIfExists } from "@/shared/lib/fs.js";

export interface LockMetadata {
  readonly pid: number;
  readonly createdAt: string;
}

export function serializeLockMetadata(): string {
  const metadata: LockMetadata = {
    pid: process.pid,
    createdAt: new Date().toISOString(),
  };
  return `${JSON.stringify(metadata)}\n`;
}

export async function readLockMetadata(lockPath: string): Promise<LockMetadata | undefined> {
  const raw = await readText(lockPath);
  if (!raw) return undefined;
  try {
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    if (typeof parsed.pid !== "number" || !Number.isInteger(parsed.pid)) {
      return undefined;
    }
    if (typeof parsed.createdAt !== "string") {
      return undefined;
    }
    return { pid: parsed.pid, createdAt: parsed.createdAt };
  } catch {
    return undefined;
  }
}

export function isProcessAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    const errno = error as NodeJS.ErrnoException;
    if (errno.code === "ESRCH") {
      return false;
    }
    if (errno.code === "EPERM") {
      return true;
    }
    return false;
  }
}

/**
 * Remove a stale `wx`-style lockfile when its mtime is older than `staleMs`
 * and the owning pid (from the lock metadata) is no longer alive.
 *
 * Math.max guards against NFS/network-mount clock skew that can report an
 * mtime in the future — without it the raw delta goes negative and the lock
 * is pinned indefinitely as "fresh".
 */
export async function removeStaleLock(lockPath: string, staleMs: number): Promise<boolean> {
  try {
    const lockStat = await stat(lockPath);
    const ageMs = Math.max(0, Date.now() - lockStat.mtimeMs);
    if (ageMs < staleMs) {
      return false;
    }
    const metadata = await readLockMetadata(lockPath);
    if (metadata && isProcessAlive(metadata.pid)) {
      return false;
    }
    await removeIfExists(lockPath);
    return true;
  } catch (error) {
    const errno = error as NodeJS.ErrnoException;
    if (errno.code === "ENOENT") {
      return false;
    }
    throw error;
  }
}

export interface FileLockOptions {
  readonly lockPath: string;
  readonly staleMs: number;
  readonly timeoutMs: number;
  readonly initialRetryDelayMs: number;
  readonly maxRetryDelayMs: number;
  readonly timeoutMessage: string;
  readonly timeoutHints: readonly string[];
}

export async function withFileLock<T>(
  options: FileLockOptions,
  fn: () => Promise<T>,
): Promise<T> {
  const deadline = Date.now() + options.timeoutMs;
  let retryDelayMs = options.initialRetryDelayMs;

  while (true) {
    try {
      const handle = await open(options.lockPath, "wx");
      try {
        await handle.writeFile(serializeLockMetadata());
        return await fn();
      } finally {
        await handle.close();
        await removeIfExists(options.lockPath);
      }
    } catch (error) {
      const errno = error as NodeJS.ErrnoException;
      if (errno.code !== "EEXIST") {
        throw error;
      }
      if (await removeStaleLock(options.lockPath, options.staleMs)) {
        continue;
      }
      if (Date.now() >= deadline) {
        throw new MaestroError(options.timeoutMessage, [...options.timeoutHints]);
      }
      await sleep(retryDelayMs);
      retryDelayMs = Math.min(retryDelayMs * 2, options.maxRetryDelayMs);
    }
  }
}
