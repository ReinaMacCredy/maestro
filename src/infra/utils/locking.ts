/**
 * Lock acquisition/release utilities for maestroCLI.
 * Extracted from paths.ts -- file-level locking with stale detection.
 */

import * as path from 'path';
import * as fs from 'fs';
import { ensureDir, readJson } from './fs-io.ts';

/** Check if a process is still running. */
function isProcessAlive(pid: number): boolean {
  try { process.kill(pid, 0); return true; } catch { return false; }
}

/** Node-compatible synchronous sleep (replaces Bun.sleepSync). */
function sleepSync(ms: number): void {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

export interface LockOptions {
  timeout?: number;
  retryInterval?: number;
  staleLockTTL?: number;
}

const DEFAULT_LOCK_OPTIONS: Required<LockOptions> = {
  timeout: 5000,
  retryInterval: 50,
  staleLockTTL: 30000,
};

export function getLockPath(filePath: string): string {
  return `${filePath}.lock`;
}

function isLockStale(lockPath: string, staleTTL: number): boolean {
  try {
    const stat = fs.statSync(lockPath);
    const age = Date.now() - stat.mtimeMs;
    if (age > staleTTL) return true;

    const content = readJson<{ pid?: number }>(lockPath);
    if (content?.pid && !isProcessAlive(content.pid)) return true;

    return false;
  } catch {
    return true;
  }
}

/**
 * Core lock acquisition loop. All fs operations are synchronous;
 * only the sleep strategy differs between async and sync callers.
 */
function acquireLockImpl(
  filePath: string,
  options: LockOptions,
  sleep: (ms: number) => void,
): () => void {
  const opts = { ...DEFAULT_LOCK_OPTIONS, ...options };
  const lockPath = getLockPath(filePath);
  const lockDir = path.dirname(lockPath);
  const startTime = Date.now();
  const lockContent = JSON.stringify({
    pid: process.pid,
    timestamp: new Date().toISOString(),
    filePath,
  });

  ensureDir(lockDir);

  while (true) {
    try {
      const fd = fs.openSync(lockPath, fs.constants.O_CREAT | fs.constants.O_EXCL | fs.constants.O_WRONLY);
      fs.writeSync(fd, lockContent);
      fs.closeSync(fd);

      return () => {
        try { fs.unlinkSync(lockPath); } catch { /* already removed */ }
      };
    } catch (err: unknown) {
      const error = err as NodeJS.ErrnoException;
      if (error.code === 'ENOENT') {
        ensureDir(lockDir);
      } else if (error.code === 'EEXIST') {
        if (isLockStale(lockPath, opts.staleLockTTL)) {
          try { fs.unlinkSync(lockPath); continue; } catch { /* race */ }
        }
      } else {
        throw error;
      }

      if (Date.now() - startTime >= opts.timeout) {
        throw new Error(
          `Failed to acquire lock on ${filePath} after ${opts.timeout}ms. Lock file: ${lockPath}`
        );
      }

      sleep(opts.retryInterval);
    }
  }
}

export function acquireLockSync(
  filePath: string,
  options: LockOptions = {}
): () => void {
  return acquireLockImpl(filePath, options, sleepSync);
}

