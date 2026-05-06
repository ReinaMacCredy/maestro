import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import type { Contract } from "@/features/task/index.js";
import type { TrustFinding } from "../../domain/types.js";

/** Substrate-managed metadata that maestro itself writes during the task
 *  lifecycle (contract creation, task heartbeat, NOW.md refresh). These
 *  files appear in the diff between lock-commit and HEAD by construction —
 *  they're not user code, so gating them with the user's scope produces
 *  false positives that block legitimate brownfield workflows. */
function isMaestroSubstratePath(path: string): boolean {
  return path === ".maestro" || path.startsWith(".maestro/");
}

/**
 * Checks every changed path against the contract scope.
 *
 * - Paths in filesForbidden → error finding.
 * - Paths outside filesExpected (when filesExpected is not ["**"] / empty) → error finding.
 * - Paths under `.maestro/` are exempt: they are substrate-managed metadata, not user code.
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
