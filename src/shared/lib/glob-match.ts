import { normalizeSlashes } from "./path-normalize.js";

const globCache = new Map<string, Bun.Glob>();

export function matchGlob(pattern: string, path: string): boolean {
  const normalized = normalizeSlashes(pattern);
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
