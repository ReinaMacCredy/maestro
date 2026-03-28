import type { CassPort } from "../ports/cass.port.js";
import type { GitPort } from "../ports/git.port.js";
import type { ConfigPort } from "../ports/config.port.js";
import type { DoctorCheck } from "../domain/types.js";

export async function runDoctor(
  cass: CassPort,
  git: GitPort,
  config: ConfigPort,
  dir: string,
): Promise<DoctorCheck[]> {
  const checks: DoctorCheck[] = [];

  // Check git
  const gitAvailable = await git.isRepo(dir);
  checks.push({
    name: "git",
    status: gitAvailable ? "ok" : "fail",
    message: gitAvailable
      ? "Git repository detected"
      : "Not inside a git repository",
    fix: gitAvailable ? undefined : "Run: git init",
  });

  // Check CASS
  const cassAvailable = await cass.isAvailable();
  checks.push({
    name: "cass",
    status: cassAvailable ? "ok" : "fail",
    message: cassAvailable
      ? "CASS is available and healthy"
      : "CASS is not installed or not responding",
    fix: cassAvailable
      ? undefined
      : "Install: brew install dicklesworthstone/tap/cass",
  });

  // Check project config
  const projectConfig = await config.exists("project", dir);
  checks.push({
    name: "project-config",
    status: projectConfig ? "ok" : "warn",
    message: projectConfig
      ? "Project config found at .maestro/config.yaml"
      : "No project config found",
    fix: projectConfig ? undefined : "Run: maestro init",
  });

  // Check global config
  const globalConfig = await config.exists("global", dir);
  checks.push({
    name: "global-config",
    status: globalConfig ? "ok" : "warn",
    message: globalConfig
      ? "Global config found at ~/.maestro/config.yaml"
      : "No global config found",
    fix: globalConfig ? undefined : "Run: maestro init --global",
  });

  return checks;
}
