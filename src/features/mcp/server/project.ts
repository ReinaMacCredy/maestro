import fs from "node:fs";
import path from "node:path";

export function findMaestroProjectRoot(
  start: string = process.cwd(),
  env: NodeJS.ProcessEnv = process.env,
): string {
  const explicit = env.MAESTRO_PROJECT_ROOT;
  if (explicit !== undefined && explicit !== "") {
    // When the env var is set, it is authoritative. Falling back to CWD
    // traversal would silently operate on a different project root than the
    // caller asked for (monorepo sub-projects are the common case), so a
    // mismatch must be a hard error.
    if (!isMaestroRoot(explicit)) {
      throw new Error(
        `MAESTRO_PROJECT_ROOT=${explicit} is not a maestro project (no .maestro/ directory found there). ` +
          "Unset the env var to fall back to upward search, or point it at a maestro-initialized repo.",
      );
    }
    return fs.realpathSync(explicit);
  }

  let current = path.resolve(start);
  while (current !== path.dirname(current)) {
    if (isMaestroRoot(current)) {
      return fs.realpathSync(current);
    }
    current = path.dirname(current);
  }

  throw new Error(
    "Not in a maestro project (no .maestro/ directory found). " +
      "Set MAESTRO_PROJECT_ROOT or cd into a maestro-initialized repo.",
  );
}

function isMaestroRoot(dir: string): boolean {
  const candidate = path.join(dir, ".maestro");
  try {
    return fs.statSync(candidate).isDirectory();
  } catch {
    return false;
  }
}
