import { normalizeSlashes } from "@/shared/lib/path-normalize.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import type { TrustFinding } from "../../domain/types.js";

// Per-pattern Glob cache mirrors the pattern used in
// src/features/task/domain/contract/verdict.ts to avoid re-allocating on
// every cross-product lookup.
const globCache = new Map<string, Bun.Glob>();

function matchGlob(pattern: string, path: string): boolean {
  const normalized = normalizeSlashes(pattern);
  let glob = globCache.get(normalized);
  if (!glob) {
    glob = new Bun.Glob(normalized);
    globCache.set(normalized, glob);
  }
  return glob.match(path);
}

function matchesAny(patterns: readonly string[], path: string): boolean {
  return patterns.some((p) => matchGlob(p, path));
}

/**
 * Checks every changed path against the contract scope.
 *
 * - Paths in filesForbidden → error finding.
 * - Paths outside filesExpected (when filesExpected is not ["**"] / empty) → error finding.
 */
export function checkScope(
  changedPaths: readonly string[],
  contract: Contract,
): readonly TrustFinding[] {
  const scope = contract.scope;
  const allowAll =
    scope.filesExpected.length === 0 ||
    (scope.filesExpected.length === 1 && scope.filesExpected[0] === "**");

  const forbidden = changedPaths.filter((p) => matchesAny(scope.filesForbidden, p));
  const outOfScope = allowAll
    ? []
    : changedPaths.filter(
        (p) =>
          !matchesAny(scope.filesForbidden, p) && !matchesAny(scope.filesExpected, p),
      );

  const findings: TrustFinding[] = [];

  if (forbidden.length > 0) {
    findings.push({
      check: "scope",
      severity: "error",
      paths: forbidden,
      details: `${forbidden.length} path(s) match filesForbidden.`,
    });
  }

  if (outOfScope.length > 0) {
    findings.push({
      check: "scope",
      severity: "error",
      paths: outOfScope,
      details: `${outOfScope.length} path(s) are outside filesExpected scope.`,
    });
  }

  return findings;
}
