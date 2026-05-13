import { parse, stringify } from "yaml";
import { MaestroError } from "@/shared/errors.js";

export function parseYaml<T = unknown>(content: string): T {
  return parse(content) as T;
}

export function stringifyYaml(data: unknown): string {
  return stringify(data);
}

/**
 * Parse a policy YAML file and validate it is a top-level object. On parse
 * failure, throws a MaestroError that includes the failing line number when
 * the underlying yaml library exposes one.
 */
export function parsePolicyYaml<T extends Record<string, unknown>>(
  text: string,
  filename: string,
): T {
  let raw: unknown;
  try {
    raw = parse(text) ?? {};
  } catch (err: unknown) {
    const yamlErr = err as { linePos?: Array<{ line: number }> };
    const line = yamlErr.linePos?.[0]?.line;
    const lineInfo = line !== undefined ? ` at line ${line}` : "";
    const msg = err instanceof Error ? err.message : String(err);
    throw new MaestroError(`${filename} malformed${lineInfo}: ${msg}`, [
      "Fix the YAML syntax and re-run",
    ]);
  }

  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    throw new MaestroError(`${filename} malformed: expected top-level object`, [
      `Got ${Array.isArray(raw) ? "array" : typeof raw}`,
    ]);
  }

  return raw as T;
}

/**
 * Deep merge two objects. Source values override target values.
 * Arrays are replaced, not concatenated.
 */
export function deepMerge<T extends Record<string, unknown>>(
  target: T,
  source: Partial<T>,
): T {
  const result = { ...target };

  for (const key of Object.keys(source) as Array<keyof T>) {
    const sourceVal = source[key];
    const targetVal = target[key];

    if (
      isPlainObject(sourceVal) &&
      isPlainObject(targetVal)
    ) {
      (result as Record<string, unknown>)[key as string] = deepMerge(
        targetVal as Record<string, unknown>,
        sourceVal as Record<string, unknown>,
      );
    } else if (sourceVal !== undefined) {
      (result as Record<string, unknown>)[key as string] = sourceVal;
    }
  }

  return result;
}

function isPlainObject(val: unknown): val is Record<string, unknown> {
  return typeof val === "object" && val !== null && !Array.isArray(val);
}
