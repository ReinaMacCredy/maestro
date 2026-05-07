export interface DetectCrossTaskConflictInput {
  readonly thisPrFiles: readonly string[];
  readonly otherPrs: readonly { readonly pr: number; readonly files: readonly string[] }[];
}

export interface DetectCrossTaskConflictResult {
  readonly conflictingPrs: readonly number[];
  readonly overlappingPaths: readonly string[]; // sorted, deduplicated
}

/**
 * Pure function — no I/O.
 *
 * A path overlaps if it appears in thisPrFiles AND in any otherPrs[i].files.
 * Conflicting PRs are those with at least one overlapping path.
 * overlappingPaths is sorted and deduplicated across all conflicting PRs.
 */
export function detectCrossTaskConflict(
  input: DetectCrossTaskConflictInput,
): DetectCrossTaskConflictResult {
  const thisFileSet = new Set(input.thisPrFiles);
  const conflictingPrs: number[] = [];
  const overlappingPathSet = new Set<string>();

  for (const other of input.otherPrs) {
    let hasOverlap = false;
    for (const file of other.files) {
      if (thisFileSet.has(file)) {
        overlappingPathSet.add(file);
        hasOverlap = true;
      }
    }
    if (hasOverlap) {
      conflictingPrs.push(other.pr);
    }
  }

  return {
    conflictingPrs,
    overlappingPaths: Array.from(overlappingPathSet).sort(),
  };
}
