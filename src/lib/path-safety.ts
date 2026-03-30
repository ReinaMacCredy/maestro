import { resolve, sep } from "node:path";
import { MaestroError } from "../domain/errors.js";

export function assertSafeSegment(
  value: string,
  label: string,
  pattern: RegExp,
  allowedDescription: string,
): void {
  if (!pattern.test(value)) {
    throw new MaestroError(`Invalid ${label}: ${value}`, [
      `${label} may only contain ${allowedDescription}`,
      `${label} must not include path separators or '..' segments`,
    ]);
  }
}

export function resolveWithin(
  root: string,
  relativePath: string,
  label: string,
): string {
  const resolvedRoot = resolve(root);
  const resolvedPath = resolve(root, relativePath);

  if (resolvedPath !== resolvedRoot && !resolvedPath.startsWith(`${resolvedRoot}${sep}`)) {
    throw new MaestroError(`${label} resolves outside the allowed root`, [
      `Root: ${resolvedRoot}`,
      `Resolved path: ${resolvedPath}`,
    ]);
  }

  return resolvedPath;
}
