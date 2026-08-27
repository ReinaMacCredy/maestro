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
