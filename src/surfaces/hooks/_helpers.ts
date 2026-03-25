import * as fs from 'node:fs';
import * as path from 'node:path';
import { findProjectRoot } from '../../features/detection.ts';
import { ensureDir } from '../../core/fs-io.ts';

/** Parse JSON from stdin. Returns {} on parse failure. */
export async function readStdin(): Promise<Record<string, unknown>> {
  return new Promise((resolve) => {
    let data = '';
    process.stdin.setEncoding('utf-8');
    process.stdin.on('data', (chunk) => { data += chunk; });
    process.stdin.on('end', () => {
      try {
        resolve(JSON.parse(data));
      } catch {
        resolve({});
      }
    });
    // Handle case where stdin is already closed
    process.stdin.on('error', () => resolve({}));
    process.stdin.resume();
  });
}

/** Write JSON output to stdout. */
export function writeOutput(data: object): void {
  console.log(JSON.stringify(data));
}

/**
 * Resolve project directory for hooks.
 * Delegates to findProjectRoot with maestroOnly + envOverride options.
 * Returns null if no .maestro/ found (not a maestro project).
 */
export function resolveProjectDir(): string | null {
  return findProjectRoot(process.cwd(), { maestroOnly: true, envOverride: true });
}

/** Hook event names -- must match keys in .claude-plugin/hooks/hooks.json. */
export const HOOK_EVENTS = {
  SessionStart: 'SessionStart',
  PreToolUse: 'PreToolUse',
  PostToolUse: 'PostToolUse',
  PreCompact: 'PreCompact',
} as const;
export type HookEventName = typeof HOOK_EVENTS[keyof typeof HOOK_EVENTS];

import { getSessionsDir, EVENTS_FILE } from '../../core/paths.ts';
export { getSessionsDir, EVENTS_FILE };

/** Log error to hook error log (best-effort). */
export function logHookError(projectDir: string | null, hookName: string, error: unknown): void {
  try {
    if (!projectDir) return;
    const logDir = getSessionsDir(projectDir);
    ensureDir(logDir);
    const logPath = path.join(logDir, 'hook-errors.log');
    const entry = `[${new Date().toISOString()}] ${hookName}: ${error instanceof Error ? error.message : String(error)}\n`;
    fs.appendFileSync(logPath, entry);
  } catch {
    // Best effort -- never throw from error logging
  }
}
