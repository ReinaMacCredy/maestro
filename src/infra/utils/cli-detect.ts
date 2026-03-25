/**
 * CLI availability detection.
 * Extracted from services.ts for reuse in conditional tool registration.
 */

import { execFileSync } from 'node:child_process';

const cache = new Map<string, boolean>();

/**
 * Check whether a CLI tool is available on PATH.
 * Returns true if the command exists, false otherwise.
 */
export function checkCli(name: string): boolean {
  const cached = cache.get(name);
  if (cached !== undefined) return cached;
  try {
    execFileSync('command', ['-v', name], { stdio: 'pipe', shell: true });
    cache.set(name, true);
    return true;
  } catch {
    cache.set(name, false);
    return false;
  }
}
