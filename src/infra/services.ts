import type { ConfigPort } from "./ports/config.port.js";
import type { GitPort } from "./ports/git.port.js";
import { YamlConfigAdapter } from "./adapters/config.adapter.js";
import { ShellGitAdapter } from "./adapters/git.adapter.js";

export interface InfraServices {
  readonly config: ConfigPort;
  readonly git: GitPort;
}

/**
 * Build the infra-layer services (config and git plumbing). The
 * current adapters do not take a projectDir argument -- they resolve
 * cwd per-call -- so the parameter is accepted for symmetry with
 * feature service builders but ignored.
 */
export function buildInfraServices(_projectDir: string): InfraServices {
  return {
    config: new YamlConfigAdapter(),
    git: new ShellGitAdapter(),
  };
}
