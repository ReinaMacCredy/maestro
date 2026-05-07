// No port adapters at this phase (L6.2).
// The auto-merge eligibility use-case is a pure function; all dependencies
// are passed in by the caller via existing service groups.
export interface MergeServices {
  // Reserved for future port adapters (L6.x).
}

export function buildMergeServices(): MergeServices {
  return {};
}
