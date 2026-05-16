import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import { isMaestroSubstratePath } from "@/shared/lib/substrate-paths.js";
import type { Contract } from "@/types/contract.js";
import type { TrustFinding } from "@/types/trust.js";

/**
 * Checks every changed path against the contract scope.
 *
 * - Paths in filesForbidden → error finding.
 * - Paths outside filesExpected (when filesExpected is not ["**"] / empty) → error finding.
 * - Maestro substrate paths (`.maestro/`, bundled `maestro:` skill bundles
 *   under `.claude/skills/` and `.codex/skills/`) are exempt — see
 *   `isMaestroSubstratePath`.
 */
export function checkScope(
  changedPaths: readonly string[],
  contract: Contract,
): readonly TrustFinding[] {
  const scope = contract.scope;
  const allowAll =
    scope.filesExpected.length === 0 ||
    (scope.filesExpected.length === 1 && scope.filesExpected[0] === "**");

  const auditable = changedPaths.filter((p) => !isMaestroSubstratePath(p));

  const forbidden = auditable.filter((p) => matchesAnyGlob(scope.filesForbidden, p));
  const outOfScope = allowAll
    ? []
    : auditable.filter(
        (p) =>
          !matchesAnyGlob(scope.filesForbidden, p) && !matchesAnyGlob(scope.filesExpected, p),
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
