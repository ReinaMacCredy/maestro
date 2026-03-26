/**
 * CLI availability detection.
 * Extracted from services.ts for reuse in conditional tool registration.
 */

import { execFileSync } from 'node:child_process';
import { MaestroError } from '../../domain/errors.ts';

/** Characters that enable shell injection when passed to sh -c. */
const SHELL_METACHARACTERS = /[;|&$`()><\n\r]/;

/** Valid binary/tool name: alphanumeric, dots, hyphens, underscores. */
const SAFE_NAME = /^[a-zA-Z0-9._-]+$/;

/**
 * Validate that a CLI tool name contains no shell metacharacters.
 * Throws MaestroError if the name is unsafe.
 */
function validateCliName(name: string): void {
  if (!SAFE_NAME.test(name)) {
    throw new MaestroError(
      `Invalid CLI tool name: '${name}'`,
      ['Tool names must match /^[a-zA-Z0-9._-]+$/'],
    );
  }
}

/**
 * Sanitize a detect command from a tool manifest before shell execution.
 * Only allows simple patterns: `<binary> --version`, `command -v <binary>`,
 * `which <binary>`, or a bare binary name.
 * Throws MaestroError if the command contains shell metacharacters.
 */
export function sanitizeDetectCommand(cmd: string): string {
  const trimmed = cmd.trim();
  if (trimmed.length === 0) {
    throw new MaestroError(
      'Empty detect command',
      ['manifest.detect must be a non-empty string or null'],
    );
  }
  if (SHELL_METACHARACTERS.test(trimmed)) {
    throw new MaestroError(
      `Unsafe detect command rejected: '${trimmed}'`,
      [
        'Detect commands must not contain shell metacharacters: ; | & $ ` ( ) > <',
        'Use simple patterns like: <binary> --version, command -v <binary>',
      ],
    );
  }
  return trimmed;
}

const cache = new Map<string, boolean>();

/**
 * Check whether a CLI tool is available on PATH.
 * Returns true if the command exists, false otherwise.
 */
export function checkCli(name: string): boolean {
  const cached = cache.get(name);
  if (cached !== undefined) return cached;
  validateCliName(name);
  try {
    execFileSync('/bin/sh', ['-c', 'command -v -- "$1"', '--', name], { stdio: 'pipe' });
    cache.set(name, true);
    return true;
  } catch {
    cache.set(name, false);
    return false;
  }
}
