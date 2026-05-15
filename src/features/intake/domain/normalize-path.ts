/**
 * Normalize an intake path so glob matching, feature-root detection, and
 * existence checks treat equivalent forms the same way:
 *
 *   "./skills/foo"               -> "skills/foo"
 *   "/Users/x/repo/.maestro/y"   -> ".maestro/y"  (when cwd = /Users/x/repo)
 *   "  src/foo.ts  "             -> "src/foo.ts"
 *
 * Paths outside `cwd` and bare relative paths are returned unchanged.
 */
export function normalizeIntakePath(path: string, cwd: string): string {
  let p = path.trim();
  if (p.startsWith("./")) p = p.slice(2);
  const cwdWithSlash = cwd.endsWith("/") ? cwd : cwd + "/";
  if (p.startsWith(cwdWithSlash)) p = p.slice(cwdWithSlash.length);
  return p;
}
