import { existsSync, realpathSync } from "node:fs";
import { homedir } from "node:os";
import { isAbsolute, resolve } from "node:path";
import { CliError } from "../kernel/cli.ts";

export function resolveHomeDirectory(options: {
  environmentHome?: string;
  fallbackHome?: string;
} = {}): string {
  const environmentHome = "environmentHome" in options
    ? options.environmentHome
    : process.env.HOME;
  const candidate = environmentHome === undefined
    ? (options.fallbackHome ?? homedir())
    : environmentHome;
  if (!candidate.trim() || !isAbsolute(candidate)) {
    throw new CliError(
      "HOME_REQUIRED",
      "HOME must resolve to an absolute path for machine-scoped Maestro state",
      { home: candidate },
    );
  }
  return resolve(candidate);
}

// The Hub room is the machine-scoped store under $HOME/maestro (d775): the
// global memory source of truth and the second store maestro search reads.
export function resolveHubRoom(home = resolveHomeDirectory()): { room: string; storePath: string } {
  const room = resolve(home, "maestro");
  return { room, storePath: resolve(room, ".maestro", "maestro.db") };
}

// A store path from process.cwd() is real while $HOME may reach it through a
// symlink (macOS tmp dirs), so path identity is decided on real paths.
export function samePath(left: string, right: string): boolean {
  const real = (path: string) => {
    try {
      return existsSync(path) ? realpathSync(path) : resolve(path);
    } catch {
      return resolve(path);
    }
  };
  return real(left) === real(right);
}
