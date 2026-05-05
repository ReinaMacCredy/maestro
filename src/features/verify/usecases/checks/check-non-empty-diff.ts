import type { TrustFinding } from "../../domain/types.js";

/**
 * Surfaces a warn finding when the diff between base and head is empty.
 *
 * Without this, an agent that staged but never committed (or that ran verify
 * before any code was written) would see "Trust Verifier: no findings" — the
 * other six checks all return clean trivially when there's nothing to inspect,
 * and the verdict would bind itself to the empty-tree SHA. The user gets a
 * healthy-looking trail backed by no actual evidence.
 */
export function checkNonEmptyDiff(
  diff: {
    readonly changedPaths: readonly string[];
    readonly addedLines: readonly string[];
    readonly base: string;
    readonly head: string;
  },
): readonly TrustFinding[] {
  if (diff.changedPaths.length > 0 || diff.addedLines.length > 0) return [];
  return [
    {
      check: "empty-diff",
      severity: "warn",
      paths: [],
      details:
        `Diff between ${diff.base} and ${diff.head} is empty. ` +
        "Stage and commit your changes before verifying — the verifier has nothing to inspect.",
    },
  ];
}
