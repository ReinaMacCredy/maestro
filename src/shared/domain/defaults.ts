import { homedir } from "node:os";
import { join } from "node:path";

export const MAESTRO_DIR = ".maestro";

export const MEMORY_DIR = "memory";

export const GRAPH_DIR = join(homedir(), ".maestro", "graph");

export const SKILLS_DIR = "skills";

/**
 * Resolve the maestro home directory. `MAESTRO_HOME` env var overrides the
 * default of `~/.maestro/` for testing and parity with `CODEX_HOME`. When
 * `homeDir` is provided (tests), it's used as the fallback root.
 */
export function resolveMaestroHome(homeDir = homedir()): string {
  return process.env["MAESTRO_HOME"] ?? join(homeDir, MAESTRO_DIR);
}

export function resolveMaestroSkillsRoot(homeDir = homedir()): string {
  return join(resolveMaestroHome(homeDir), SKILLS_DIR);
}

/**
 * Resolve the Codex CLI home directory. `CODEX_HOME` env var is the official
 * Codex override (per OpenAI docs); fall back to `~/.codex/`.
 */
export function resolveCodexHome(homeDir = homedir()): string {
  return process.env["CODEX_HOME"] ?? join(homeDir, ".codex");
}
