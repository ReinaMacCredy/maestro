/**
 * Pure gate-check evaluator for behavioral principles.
 * Shared utility so both mission validators and handoff commands
 * can evaluate gate conditions without cross-feature imports.
 */

export function evaluateGateCheck(checkType: string, value: unknown): boolean {
  if (checkType === "object_non_empty") {
    return isNonEmptyObject(value);
  }

  if (checkType === "array_all_passed") {
    return isArrayAllPassed(value);
  }

  const minLengthMatch = checkType.match(/^array_min_length:(\d+)$/);
  if (minLengthMatch) {
    const minLength = Number(minLengthMatch[1]);
    return Array.isArray(value) && value.length >= minLength;
  }

  return false;
}

function isNonEmptyObject(value: unknown): boolean {
  if (value === null || value === undefined) return false;
  if (typeof value !== "object" || Array.isArray(value)) return false;
  return Object.keys(value as Record<string, unknown>).length > 0;
}

function isArrayAllPassed(value: unknown): boolean {
  if (!Array.isArray(value) || value.length === 0) return false;
  return value.every(
    (item) =>
      item !== null &&
      typeof item === "object" &&
      "passed" in (item as Record<string, unknown>) &&
      (item as Record<string, unknown>).passed === true,
  );
}
