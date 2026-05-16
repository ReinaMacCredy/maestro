import type { TrustFinding } from "@/types/trust.js";

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
  // When base == head the typical cause is "locked the contract, then ran
  // verify before committing any work after lock." Pointing at staging is
  // misleading there — what's needed is a fresh commit AFTER the lock.
  const baseEqualsHead = diff.base === diff.head;
  const details = baseEqualsHead
    ? `Diff between ${diff.base} and ${diff.head} is empty (base equals HEAD). ` +
      "Commit work after locking the contract — the verifier diffs from the lock-commit."
    : `Diff between ${diff.base} and ${diff.head} is empty. ` +
      "Stage and commit your changes before verifying — the verifier has nothing to inspect.";
  return [
    {
      check: "empty-diff",
      severity: "warn",
      paths: [],
      details,
    },
  ];
}
