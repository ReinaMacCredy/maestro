/**
 * Name validation for maestroCLI.
 * Forked from claude-maestro/src/utils/resolve.ts validateName function.
 */

export function validateName(raw: string, label = 'name'): { ok: true; name: string } | { ok: false; error: string } {
  const trimmed = raw.trim();
  if (trimmed.length === 0) {
    return { ok: false, error: `${label} cannot be empty` };
  }
  if (trimmed.length > 128) {
    return { ok: false, error: `${label} too long (max 128 chars)` };
  }
  // Reject path traversal components
  if (/(?:^|[\\/])\.\.(?:[\\/]|$)/.test(trimmed) || trimmed.includes('\0')) {
    return { ok: false, error: `${label} contains invalid path components` };
  }
  // Reject slashes, backslashes, and control characters
  if (/[/\\]/.test(trimmed)) {
    return { ok: false, error: `${label} cannot contain slashes` };
  }
  if (/[\x00-\x1f\x7f]/.test(trimmed)) {
    return { ok: false, error: `${label} contains control characters` };
  }
  // Reject names that are just dots
  if (/^\.+$/.test(trimmed)) {
    return { ok: false, error: `${label} cannot be "." or ".."` };
  }
  // Reject whitespace in names (causes directory issues)
  if (/\s/.test(trimmed)) {
    return { ok: false, error: `${label} cannot contain whitespace` };
  }
  return { ok: true, name: trimmed };
}
