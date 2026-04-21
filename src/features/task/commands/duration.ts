import { MaestroError } from "@/shared/errors.js";

const DURATION_PATTERN = /^(\d+)(ms|s|m|h|d)$/;

export function parseDuration(value: string, flag: string): number {
  const m = DURATION_PATTERN.exec(value.trim());
  if (!m) {
    throw new MaestroError(`Invalid ${flag} '${value}'`, [
      "Duration must be a positive integer with a suffix: ms, s, m, h, or d",
      "Examples: 30s, 15m, 4h, 2d",
    ]);
  }
  const n = Number(m[1]);
  const unit = m[2];
  switch (unit) {
    case "ms": return n;
    case "s": return n * 1000;
    case "m": return n * 60_000;
    case "h": return n * 3_600_000;
    case "d": return n * 86_400_000;
  }
  throw new MaestroError(`Invalid ${flag} '${value}'`, []);
}
