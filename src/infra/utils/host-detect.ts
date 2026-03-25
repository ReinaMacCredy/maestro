/**
 * Host environment detection.
 * Detects whether maestro is running inside claude-code, codex, or standalone.
 */

export type HostType = 'claude-code' | 'codex' | 'standalone';

let _cached: HostType | undefined;

/** Detect the host environment from env vars. Result is cached. */
export function detectHost(): HostType {
  if (_cached !== undefined) return _cached;

  if (process.env.CLAUDE_PROJECT_DIR || process.env.CLAUDE_SESSION_ID) {
    _cached = 'claude-code';
  } else if (process.env.CODEX_CI || process.env.CODEX_THREAD_ID) {
    _cached = 'codex';
  } else {
    _cached = 'standalone';
  }

  return _cached;
}

/** Whether maestro is running inside a host environment (not standalone). */
export function isHosted(): boolean {
  return detectHost() !== 'standalone';
}

/** Reset the cached detection (for testing). */
export function _resetHostDetection(): void {
  _cached = undefined;
}
