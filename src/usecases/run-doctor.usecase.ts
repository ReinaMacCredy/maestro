import type { CassPort } from "../ports/cass.port.js";
import type { GitPort } from "../ports/git.port.js";
import type { ConfigPort } from "../ports/config.port.js";
import type { DoctorCheck } from "../domain/types.js";
import { CASS_INSTALL_HINT } from "../domain/defaults.js";

export async function runDoctor(
  cass: CassPort,
  git: GitPort,
  config: ConfigPort,
  dir: string,
): Promise<DoctorCheck[]> {
  const [gitAvailable, cassAvailable, projectConfig, globalConfig] =
    await Promise.all([
      git.isRepo(dir),
      cass.isAvailable(),
      config.exists("project", dir),
      config.exists("global", dir),
    ]);

  return [
    {
      name: "git",
      status: gitAvailable ? "ok" : "fail",
      message: gitAvailable ? "Git repository detected" : "Not inside a git repository",
      fix: gitAvailable ? undefined : "Run: git init",
    },
    {
      name: "cass",
      status: cassAvailable ? "ok" : "fail",
      message: cassAvailable ? "CASS is available and healthy" : "CASS is not installed or not responding",
      fix: cassAvailable ? undefined : CASS_INSTALL_HINT,
    },
    {
      name: "project-config",
      status: projectConfig ? "ok" : "warn",
      message: projectConfig ? "Project config found at .maestro/config.yaml" : "No project config found",
      fix: projectConfig ? undefined : "Run: maestro init",
    },
    {
      name: "global-config",
      status: globalConfig ? "ok" : "warn",
      message: globalConfig ? "Global config found at ~/.maestro/config.yaml" : "No global config found",
      fix: globalConfig ? undefined : "Run: maestro init --global",
    },
  ];
}
