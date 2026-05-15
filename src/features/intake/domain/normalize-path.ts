import { normalize as posixNormalize } from "node:path/posix";

/**
 * Normalize an intake path so glob matching, feature-root detection, and
 * existence checks treat equivalent forms the same way:
 *
 *   "./skills/foo"               -> "skills/foo"
 *   ".//skills/foo"              -> "skills/foo"     (collapses double slashes)
 *   "./.maestro/x"               -> ".maestro/x"     (collapses leading dot-slash)
 *   "/Users/x/repo/.maestro/y"   -> ".maestro/y"     (when cwd = /Users/x/repo)
 *   "  src/foo.ts  "             -> "src/foo.ts"
 *
 * Paths outside `cwd` and bare relative paths are returned otherwise unchanged.
 */
export function normalizeIntakePath(path: string, cwd: string): string {
  const trimmed = path.trim();
  if (trimmed.length === 0) return trimmed;

  const cwdWithSlash = cwd.endsWith("/") ? cwd : cwd + "/";
  const stripped = trimmed.startsWith(cwdWithSlash)
    ? trimmed.slice(cwdWithSlash.length)
    : trimmed;

  if (stripped.startsWith("/")) return stripped;
  return posixNormalize(stripped);
}
