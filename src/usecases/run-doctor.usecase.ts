import type { CassPort } from "../ports/cass.port.js";
import type { GitPort } from "../ports/git.port.js";
import type { ConfigPort } from "../ports/config.port.js";
import { listIgnoredProjectConfigKeys } from "../domain/ui-config.js";
import type { DoctorCheck } from "../domain/types.js";
import { CASS_INSTALL_HINT } from "../domain/defaults.js";

export async function runDoctor(
  cass: CassPort,
  git: GitPort,
  config: ConfigPort,
  dir: string,
): Promise<DoctorCheck[]> {
  const [gitAvailable, cassAvailable, projectConfig, globalConfig, configLayers] =
    await Promise.all([
      git.isRepo(dir),
      cass.isAvailable(),
      config.exists("project", dir),
      config.exists("global", dir),
      config.loadLayers(dir),
    ]);

  const effectiveWorkers = configLayers.effective.workers ?? {};
  const defaultWorker = configLayers.effective.execution?.defaultWorker;
  const defaultWorkerConfig = defaultWorker ? effectiveWorkers[defaultWorker] : undefined;
  const doctorChecks: DoctorCheck[] = [
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

  if (defaultWorker) {
    doctorChecks.push({
      name: "default-worker",
      status: defaultWorkerConfig?.enabled ? "ok" : "fail",
      message: defaultWorkerConfig?.enabled
        ? `Default worker '${defaultWorker}' is enabled`
        : `Default worker '${defaultWorker}' is missing or disabled`,
      fix: defaultWorkerConfig?.enabled
        ? undefined
        : "Set execution.defaultWorker to an enabled worker profile",
    });
  }

  for (const [slug, worker] of Object.entries(effectiveWorkers)) {
    if (!worker.enabled || worker.transport !== "cli") {
      continue;
    }

    const available = Bun.which(worker.command);
    doctorChecks.push({
      name: `worker-${slug}`,
      status: available ? "ok" : "fail",
      message: available
        ? `Worker command '${worker.command}' is available for ${slug}`
        : `Worker command '${worker.command}' is missing for ${slug}`,
      fix: available
        ? undefined
        : `Install '${worker.command}' or disable worker '${slug}'`,
      });
    }

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
