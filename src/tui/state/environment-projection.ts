import type { ConfigPort } from "@/infra/ports/config.port.js";
import type { GitPort } from "@/infra/ports/git.port.js";
import type { MaestroConfig } from "@/infra/domain/config-types.js";
import type { DoctorCheck, EnvironmentStatus } from "@/infra/domain/status-types.js";
import { listIgnoredProjectConfigKeys } from "@/tui/shared/ui-config.js";

export async function buildMissionControlEnvironmentSummary(
  config: ConfigPort,
  git: GitPort,
  cwd: string,
): Promise<{ status: EnvironmentStatus; checks: readonly DoctorCheck[] }> {
  const [
    projectConfigExists,
    globalConfigExists,
    gitAvailable,
  ] = await Promise.all([
    config.exists("project", cwd),
    config.exists("global", cwd),
    git.isRepo(cwd),
  ]);

  const configSource: EnvironmentStatus["configSource"] = projectConfigExists
    ? "project"
    : globalConfigExists
      ? "global"
      : "none";

  return {
    status: {
      initialized: projectConfigExists || globalConfigExists,
      configSource,
      gitAvailable,
      legacyHandoffCount: 0,
    },
    checks: [
      {
        name: "git",
        status: gitAvailable ? "ok" : "fail",
        message: gitAvailable ? "Git repository detected" : "Not inside a git repository",
        fix: gitAvailable ? undefined : "Run: git init",
      },
      {
        name: "project-config",
        status: projectConfigExists ? "ok" : "warn",
        message: projectConfigExists ? "Project config found at .maestro/config.yaml" : "No project config found",
        fix: projectConfigExists ? undefined : "Run: maestro init",
      },
      {
        name: "global-config",
        status: globalConfigExists ? "ok" : "warn",
        message: globalConfigExists ? "Global config found at ~/.maestro/config.yaml" : "No global config found",
        fix: globalConfigExists ? undefined : "Run: maestro init --global",
      },
    ],
  };
}

export function buildIgnoredProjectOverrideChecks(
  projectConfig: MaestroConfig | undefined,
): DoctorCheck[] {
  return listIgnoredProjectConfigKeys(projectConfig).map((keyPath) => ({
    name: `ignored-${keyPath.replaceAll(".", "-")}`,
    status: "warn" as const,
    message: `${keyPath} is set in project config but only global config is used`,
    fix: "Remove the project value or set it in ~/.maestro/config.yaml instead",
  }));
}
