import { InvalidArgumentError } from "commander";
import { MaestroError } from "@/shared/errors.js";

// Commander's default behavior for repeated `--flag <value>` is last-wins,
// which silently hides caller uncertainty. For aliasing flags that mirror
// a positional arg (e.g. `--task <id>` on `task introspect`, `--title <t>`
// on `task create`), pair this parser with the option so the verb rejects
// repeated use with a clear "pass it once" error instead of swallowing the
// first value.
export function singletonOption(value: string, previous: unknown): string {
  if (previous !== undefined) {
    throw new InvalidArgumentError("pass it once, not multiple times");
  }
  return value;
}

export function parseNonNegativeInt(raw: string): number {
  const n = Number(raw);
  if (!Number.isFinite(n) || !Number.isInteger(n) || n < 0) {
    throw new MaestroError(`Invalid integer: ${raw}`, [
      "Pass a non-negative integer (0 or greater)",
    ]);
  }
  return n;
}

export function parsePositiveInt(raw: string): number {
  const n = Number(raw);
  if (!Number.isFinite(n) || !Number.isInteger(n) || n < 1) {
    throw new MaestroError(`Invalid integer: ${raw}`, [
      "Pass a positive integer (1 or greater)",
    ]);
  }
  return n;
}
