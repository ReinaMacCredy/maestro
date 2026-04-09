import type { GitPort } from "../ports/git.port.js";
import type { ConfigPort } from "../ports/config.port.js";
import { listIgnoredProjectConfigKeys } from "@/shared/domain/ui-config.js";
import type { DoctorCheck } from "@/infra/domain/status-types.js";

/**
 * Phase 1 strip: CASS and worker-transport checks were removed. The
 * conductor model does not spawn workers or depend on CASS, so these
 * checks no longer map to anything the CLI can fix.
 */
export async function runDoctor(
  git: GitPort,
  config: ConfigPort,
  dir: string,
): Promise<DoctorCheck[]> {
  const [gitAvailable, projectConfig, globalConfig, configLayers] =
    await Promise.all([
      git.isRepo(dir),
      config.exists("project", dir),
      config.exists("global", dir),
      config.loadLayers(dir),
    ]);

  const doctorChecks: DoctorCheck[] = [
    {
      name: "git",
      status: gitAvailable ? "ok" : "fail",
      message: gitAvailable ? "Git repository detected" : "Not inside a git repository",
      fix: gitAvailable ? undefined : "Run: git init",
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

  for (const keyPath of listIgnoredProjectConfigKeys(configLayers.project)) {
    doctorChecks.push({
      name: `ignored-${keyPath.replaceAll(".", "-")}`,
      status: "warn",
      message: `${keyPath} is set in project config but only global config is used`,
      fix: "Remove the project value or set it in ~/.maestro/config.yaml instead",
    });
  }

  return doctorChecks;
}
