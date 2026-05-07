import fs from "node:fs";
import path from "node:path";

export function findMaestroProjectRoot(
  start: string = process.cwd(),
  env: NodeJS.ProcessEnv = process.env,
): string {
  const explicit = env.MAESTRO_PROJECT_ROOT;
  if (explicit && fs.existsSync(path.join(explicit, ".maestro"))) {
    return fs.realpathSync(explicit);
  }

  let current = path.resolve(start);
  while (current !== path.dirname(current)) {
    if (fs.existsSync(path.join(current, ".maestro"))) {
      return fs.realpathSync(current);
    }
    current = path.dirname(current);
  }

  throw new Error(
    "Not in a maestro project (no .maestro/ directory found). " +
      "Set MAESTRO_PROJECT_ROOT or cd into a maestro-initialized repo.",
  );
}
