import { existsSync, realpathSync } from "node:fs";
import { dirname, join, parse } from "node:path";

export function resolveMaestroProjectRoot(startDir: string): string {
  let current = safeRealpath(startDir);
  const root = parse(current).root;

  while (true) {
    if (existsSync(join(current, ".maestro")) || existsSync(join(current, ".git"))) {
      return current;
    }
    if (current === root) {
      return startDir;
    }
    current = dirname(current);
  }
}

function safeRealpath(value: string): string {
  try {
    return realpathSync(value);
  } catch {
    return value;
  }
}
