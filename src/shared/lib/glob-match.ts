import { normalizeSlashes } from "./path-normalize.js";

const globCache = new Map<string, Bun.Glob>();

const MAX_PATTERN_LENGTH = 256;
const MAX_WILDCARDS = 16;

// Bun.Glob compiles patterns to its internal matcher (not JS regex), so it
// is not subject to JS-regex catastrophic backtracking. The bounds below
// still cap memory and compile time for malformed patterns from policy
// files or CLI args.
export function matchGlob(pattern: string, path: string): boolean {
  const normalized = normalizeSlashes(pattern);
  if (normalized.length > MAX_PATTERN_LENGTH) return false;
  let wildcards = 0;
  for (let i = 0; i < normalized.length; i++) {
    const ch = normalized.charCodeAt(i);
    if (ch === 42 /* * */ || ch === 63 /* ? */) {
      wildcards++;
      if (wildcards > MAX_WILDCARDS) return false;
    }
  }
  let glob = globCache.get(normalized);
  if (!glob) {
    glob = new Bun.Glob(normalized);
    globCache.set(normalized, glob);
  }
  return glob.match(path);
}

export function matchesAnyGlob(patterns: readonly string[], path: string): boolean {
  return patterns.some((p) => matchGlob(p, path));
}
